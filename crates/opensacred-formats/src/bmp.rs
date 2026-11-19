//! Windows bitmaps (`.bmp`): the loading and save screens of `PAK/` (docs/02 §11), uncompressed,
//! 1024 × 768, rows from the bottom; 24-bit in the GOG release, and one of the Steam release's
//! loading screens 16-bit (docs/01 §1, #1488). And the device-independent bitmaps of a program's
//! resources, `Sacred.exe`'s three pictures of 8 bits through a palette (#2891).

use crate::error::{ErrorKind, Result, field, to_usize};
use crate::reader::Reader;

/// A decoded bitmap: RGBA rows from the top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Decodes a `BITMAPFILEHEADER` and `BITMAPINFOHEADER` bitmap of 24 or 16 bits per pixel without
/// compression (`BI_RGB`, so 5-5-5 for 16 bits, the top bit unused), or of 8 through its palette;
/// bytes after the pixels (2 in three files of the game) are ignored.
pub fn decode(data: &[u8]) -> Result<Bitmap> {
    let file = Reader::new(data);
    let mut r = file.clone();
    let magic: [u8; 2] = r.array("BMP magic")?;
    if &magic != b"BM" {
        return Err(file.error(ErrorKind::Signature { what: "BMP" }));
    }
    r.u32()?; // the file size
    r.u32()?; // reserved
    let pixels_at = to_usize(r.u32()?);
    info(data, r.offset(), Some(pixels_at))
}

/// Decodes a device-independent bitmap: a `BITMAPINFOHEADER`, its palette and its pixels with no
/// file header, as a program's bitmap resource holds one (`Sacred.exe`'s 124 to 126, of 8 bits
/// per pixel, #2891); the pixels follow the palette.
pub fn decode_dib(data: &[u8]) -> Result<Bitmap> {
    info(data, 0, None)
}

/// The bitmap whose `BITMAPINFOHEADER` is at `header_at` of `data`, its pixels at `pixels_at`, or
/// right after its palette for none: the header, its palette of `biClrUsed` entries (all 256 of
/// 8 bits where that is 0), each blue, green, red and a byte unused, and its rows, each padded to
/// four bytes.
fn info(data: &[u8], header_at: usize, pixels_at: Option<usize>) -> Result<Bitmap> {
    let file = Reader::new(data);
    let mut r = Reader::at(data, header_at);
    let header_size = r.u32()?;
    if header_size < 40 {
        return Err(file.error_at(
            header_at,
            field("BMP header size", header_size.into(), "at least 40".into()),
        ));
    }
    let width = r.u32()?;
    let height_at = r.offset();
    let height = i32::from_le_bytes(r.array("BMP height")?);
    r.u16()?; // planes
    let bits_at = r.offset();
    let bits = r.u16()?;
    let compression = r.u32()?;
    if !matches!(bits, 8 | 16 | 24) || compression != 0 {
        return Err(file.error_at(
            bits_at,
            field(
                "BMP bits per pixel",
                bits.into(),
                "24, 16 or 8, uncompressed".into(),
            ),
        ));
    }
    // The pixels' size and the two resolutions, then the palette's count.
    r.bytes(12, "BMP header")?;
    let used = to_usize(r.u32()?);
    let colours = match (used, bits) {
        (0, 8) => 256,
        (used, _) => used,
    };
    let palette_at = header_at.saturating_add(to_usize(header_size));
    let palette = file.window(palette_at, colours.saturating_mul(4), "BMP palette")?;
    let palette = palette.rest();
    let pixels_at = pixels_at.unwrap_or(palette_at.saturating_add(palette.len()));
    let rows = height.unsigned_abs();
    if rows == 0 || width == 0 {
        return Err(file.error_at(height_at, field("BMP height", 0, "not 0".into())));
    }
    let pixel_len = usize::from(bits / 8);
    // A header's sizes may be anything: each product is checked, as `tex::image_bytes` checks its.
    let too_large = || {
        file.error_at(
            height_at,
            ErrorKind::ImageTooLarge {
                width,
                height: rows,
            },
        )
    };
    let row_len = to_usize(width)
        .checked_mul(pixel_len)
        .and_then(|len| len.checked_next_multiple_of(4))
        .ok_or_else(too_large)?;
    let pixels_len = row_len.checked_mul(to_usize(rows)).ok_or_else(too_large)?;
    let rgba_len = to_usize(width)
        .checked_mul(to_usize(rows))
        .and_then(|len| len.checked_mul(4))
        .ok_or_else(too_large)?;
    let pixels = file.window(pixels_at, pixels_len, "BMP pixels")?;
    let pixels = pixels.rest();
    let mut rgba = Vec::with_capacity(rgba_len);
    for y in 0..to_usize(rows) {
        // A positive height stores the bottom row first.
        let stored = if height > 0 {
            to_usize(rows) - 1 - y
        } else {
            y
        };
        let at = stored * row_len;
        let row = &pixels[at..at + to_usize(width) * pixel_len];
        match bits {
            24 => {
                for &[b, g, r] in row.as_chunks::<3>().0 {
                    rgba.extend_from_slice(&[r, g, b, 255]);
                }
            }
            16 => {
                for &pixel in row.as_chunks::<2>().0 {
                    let [r, g, b] = rgb555(u16::from_le_bytes(pixel));
                    rgba.extend_from_slice(&[r, g, b, 255]);
                }
            }
            _ => {
                for (x, &index) in row.iter().enumerate() {
                    let entry = usize::from(index) * 4;
                    let Some(&[b, g, r, _]) = palette
                        .get(entry..entry + 4)
                        .and_then(|e| e.first_chunk::<4>())
                    else {
                        return Err(file.error_at(
                            pixels_at + at + x,
                            field(
                                "BMP palette index",
                                index.into(),
                                format!("below {colours}"),
                            ),
                        ));
                    };
                    rgba.extend_from_slice(&[r, g, b, 255]);
                }
            }
        }
    }
    Ok(Bitmap {
        width,
        height: rows,
        rgba,
    })
}

/// The red, green and blue of a 5-5-5 pixel, blue in the low five bits, each widened to eight
/// bits by repeating its top bits, so that 31 is 255.
fn rgb555(pixel: u16) -> [u8; 3] {
    [10, 5, 0].map(|shift| {
        let five = u8::try_from((pixel >> shift) & 0x1F).unwrap_or(0);
        five << 3 | five >> 2
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2 × 2 bitmap, rows from the bottom: blue, green / red, white at the top.
    fn sample(height: i32) -> Vec<u8> {
        let mut d = b"BM".to_vec();
        d.extend_from_slice(&[0; 8]);
        d.extend_from_slice(&54u32.to_le_bytes());
        d.extend_from_slice(&40u32.to_le_bytes());
        d.extend_from_slice(&2u32.to_le_bytes());
        d.extend_from_slice(&height.to_le_bytes());
        d.extend_from_slice(&1u16.to_le_bytes());
        d.extend_from_slice(&24u16.to_le_bytes());
        d.extend_from_slice(&[0; 24]);
        // Each row is 6 bytes of pixels and 2 of padding.
        d.extend_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0]);
        d.extend_from_slice(&[0, 0, 255, 255, 255, 255, 0, 0]);
        d
    }

    #[test]
    fn rows_from_the_bottom_come_out_from_the_top() {
        let bitmap = decode(&sample(2)).unwrap();
        assert_eq!((bitmap.width, bitmap.height), (2, 2));
        assert_eq!(
            bitmap.rgba,
            [
                255, 0, 0, 255, 255, 255, 255, 255, 0, 0, 255, 255, 0, 255, 0, 255
            ]
        );
        // A negative height stores the top row first.
        let top_down = decode(&sample(-2)).unwrap();
        assert_eq!(&top_down.rgba[..4], [0, 0, 255, 255]);
    }

    #[test]
    fn sizes_past_memory_are_an_error() {
        let mut data = sample(2);
        data[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
        data[22..26].copy_from_slice(&i32::MIN.to_le_bytes());
        let error = decode(&data).unwrap_err().to_string();
        assert!(error.contains("does not fit in memory"), "{error}");
    }

    #[test]
    fn only_uncompressed_24_16_or_8_bits() {
        let mut data = sample(2);
        data[28] = 32;
        assert!(
            decode(&data)
                .unwrap_err()
                .to_string()
                .contains("`BMP bits per pixel` is 32")
        );
        let mut data = sample(2);
        data[30] = 1;
        assert!(
            decode(&data)
                .unwrap_err()
                .to_string()
                .contains("`BMP bits per pixel` is 24")
        );
    }

    /// A device-independent bitmap of 3 × 2 and 8 bits, rows from the bottom, of a palette of
    /// `used` entries (none written for 0, where the decoder takes 256): red, green, blue.
    fn dib_8(used: u32, rows: [[u8; 3]; 2]) -> Vec<u8> {
        let mut d = 40u32.to_le_bytes().to_vec();
        d.extend_from_slice(&3u32.to_le_bytes());
        d.extend_from_slice(&2i32.to_le_bytes());
        d.extend_from_slice(&1u16.to_le_bytes());
        d.extend_from_slice(&8u16.to_le_bytes());
        d.extend_from_slice(&[0; 16]);
        d.extend_from_slice(&used.to_le_bytes());
        d.extend_from_slice(&[0; 4]);
        let mut palette = vec![0, 0, 255, 0, 0, 255, 0, 0, 255, 0, 0, 0];
        palette.resize(if used == 0 { 1024 } else { 12 }, 7);
        d.extend_from_slice(&palette);
        // Each row is 3 bytes of indices and 1 of padding.
        for row in rows {
            d.extend_from_slice(&row);
            d.push(0);
        }
        d
    }

    #[test]
    fn an_8_bit_dib_takes_its_colours_from_its_palette() {
        let red = [255, 0, 0, 255];
        let green = [0, 255, 0, 255];
        let blue = [0, 0, 255, 255];
        let bitmap = decode_dib(&dib_8(3, [[0, 1, 2], [2, 2, 1]])).unwrap();
        assert_eq!((bitmap.width, bitmap.height), (3, 2));
        assert_eq!(bitmap.rgba, [blue, blue, green, red, green, blue].concat());
        // A count of 0 is all 256 entries, the pixels after them.
        let bitmap = decode_dib(&dib_8(0, [[0, 1, 2], [3, 0, 0]])).unwrap();
        assert_eq!(
            bitmap.rgba,
            [[7, 7, 7, 255], red, red, red, green, blue].concat()
        );
        // An index past the palette is an error at its pixel.
        let error = decode_dib(&dib_8(3, [[0, 1, 2], [3, 0, 0]]))
            .unwrap_err()
            .to_string();
        assert!(error.contains("`BMP palette index` is 3"), "{error}");
        assert!(error.contains("below 3"), "{error}");
    }

    #[test]
    fn a_dibs_pixels_follow_its_header_and_its_palette() {
        // A 24-bit one with a palette of one entry, which it does not use.
        let mut d = sample(2)[14..54].to_vec();
        d[32..36].copy_from_slice(&1u32.to_le_bytes());
        d.extend_from_slice(&[9; 4]);
        d.extend_from_slice(&sample(2)[54..]);
        assert_eq!(decode_dib(&d).unwrap(), decode(&sample(2)).unwrap());
        // A header larger than 40 bytes moves them on.
        let mut d = dib_8(3, [[0, 1, 2], [2, 2, 1]]);
        d[..4].copy_from_slice(&44u32.to_le_bytes());
        d.splice(40..40, [0; 4]);
        assert_eq!(
            decode_dib(&d).unwrap(),
            decode_dib(&dib_8(3, [[0, 1, 2], [2, 2, 1]])).unwrap()
        );
    }

    /// A 3 × 2 bitmap of 16 bits, rows from the bottom: blue, green, black / red, white, grey.
    fn sample_16() -> Vec<u8> {
        let mut d = b"BM".to_vec();
        d.extend_from_slice(&[0; 8]);
        d.extend_from_slice(&54u32.to_le_bytes());
        d.extend_from_slice(&40u32.to_le_bytes());
        d.extend_from_slice(&3u32.to_le_bytes());
        d.extend_from_slice(&2i32.to_le_bytes());
        d.extend_from_slice(&1u16.to_le_bytes());
        d.extend_from_slice(&16u16.to_le_bytes());
        d.extend_from_slice(&[0; 24]);
        // Each row is 6 bytes of pixels and 2 of padding; the top bit of a pixel is unused.
        for row in [[0x001F, 0x03E0, 0x8000], [0x7C00, 0x7FFF, 0x4210]] {
            for pixel in row {
                d.extend_from_slice(&u16::to_le_bytes(pixel));
            }
            d.extend_from_slice(&[0; 2]);
        }
        // Bytes after the pixels, as the Steam release's loading screens have.
        d.extend_from_slice(&[0; 2]);
        d
    }

    #[test]
    fn a_16_bit_bitmap_widens_its_five_bits_to_eight() {
        let bitmap = decode(&sample_16()).unwrap();
        assert_eq!((bitmap.width, bitmap.height), (3, 2));
        assert_eq!(
            bitmap.rgba,
            [
                255, 0, 0, 255, 255, 255, 255, 255, 132, 132, 132, 255, // the top row
                0, 0, 255, 255, 0, 255, 0, 255, 0, 0, 0, 255, // the bottom row
            ]
        );
    }
}

/// Tests on the GOG installation's `Sacred.exe`: each bitmap of its resources, found and decoded,
/// against the rows of the Python prototype `tools/analysis/formats/exe_bitmaps.py`. CI does not
/// have the game files, so the test is ignored by default.
#[cfg(test)]
mod game_tests {
    use super::*;
    use crate::pe::{BITMAP, Image};
    use crate::testing::{game_file, run_python};

    #[test]
    #[ignore = "needs the game files in OriginalGame/ and Python"]
    fn every_bitmap_of_the_executable_matches_exe_bitmaps_py() {
        let game = opensacred_locate::original();
        let rows = run_python(
            "tools/analysis/formats/exe_bitmaps.py",
            &[std::ffi::OsStr::new("--game-dir"), game.as_os_str()],
        );
        let exe = game_file("Sacred.exe");
        let image = Image::parse(&exe).unwrap();
        let mut numbers = Vec::new();
        for row in rows.lines() {
            let words: Vec<&str> = row.split_whitespace().collect();
            let number: u32 = words[0].parse().unwrap();
            let data = image.resource(BITMAP, number).unwrap();
            let bitmap = decode_dib(data).unwrap();
            let ours = format!(
                "{number} lang {} bytes {} {}x{} {} compression {} rgba {:08x}",
                words[2],
                data.len(),
                bitmap.width,
                bitmap.height,
                words[6],
                words[8],
                crc32fast::hash(&bitmap.rgba)
            );
            assert_eq!(ours, row);
            numbers.push(number);
        }
        // The three pictures of `InfoPlayer`'s mode 10 among them (#2891).
        assert!(
            [124, 125, 126].iter().all(|n| numbers.contains(n)),
            "{numbers:?}"
        );
    }
}
