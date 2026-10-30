//! Windows bitmaps (`.bmp`): the loading and save screens of `PAK/` (docs/02 §11), uncompressed,
//! 1024 × 768, rows from the bottom; 24-bit in the GOG release, and one of the Steam release's
//! loading screens 16-bit (docs/01 §1, #1488).

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
/// compression (`BI_RGB`, so 5-5-5 for 16 bits, the top bit unused); bytes after the pixels (2 in
/// three files of the game) are ignored.
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
    let header_at = r.offset();
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
    if !matches!(bits, 16 | 24) || compression != 0 {
        return Err(file.error_at(
            bits_at,
            field(
                "BMP bits per pixel",
                bits.into(),
                "24 or 16, uncompressed".into(),
            ),
        ));
    }
    let rows = height.unsigned_abs();
    if rows == 0 || width == 0 {
        return Err(file.error_at(height_at, field("BMP height", 0, "not 0".into())));
    }
    let pixel_len = usize::from(bits / 8);
    let row_len = (to_usize(width) * pixel_len).next_multiple_of(4);
    let pixels = file.window(pixels_at, row_len * to_usize(rows), "BMP pixels")?;
    let pixels = pixels.rest();
    let mut rgba = Vec::with_capacity(to_usize(width) * to_usize(rows) * 4);
    for y in 0..to_usize(rows) {
        // A positive height stores the bottom row first.
        let stored = if height > 0 {
            to_usize(rows) - 1 - y
        } else {
            y
        };
        let row = &pixels[stored * row_len..stored * row_len + to_usize(width) * pixel_len];
        if bits == 24 {
            for &[b, g, r] in row.as_chunks::<3>().0 {
                rgba.extend_from_slice(&[r, g, b, 255]);
            }
        } else {
            for &pixel in row.as_chunks::<2>().0 {
                let [r, g, b] = rgb555(u16::from_le_bytes(pixel));
                rgba.extend_from_slice(&[r, g, b, 255]);
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
    fn only_uncompressed_24_or_16_bits() {
        let mut data = sample(2);
        data[28] = 8;
        assert!(
            decode(&data)
                .unwrap_err()
                .to_string()
                .contains("`BMP bits per pixel` is 8")
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
