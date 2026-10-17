//! `PAK/mixed.pak` (`"MIX"` v0): the sprites of the static objects, each assembled from pieces of
//! `texture.pak` atlases (docs/02 §3.3). An object type of `items.pak` names its sprite; a static
//! object of `Static.PAK` puts the sprite's top left corner at its position. The layout follows
//! openheilig (engine `formats/mixed.gd`, MIT) and is confirmed on every record by
//! `tools/analysis/formats/mixed_dump.py`.

use crate::error::{Result, field, to_usize};
use crate::pak::Pak;
use crate::reader::Reader;

pub const MAGIC: [u8; 3] = *b"MIX";
pub const VERSION: u8 = 0;
const HEADER_SIZE: usize = 16;
const PIECE_SIZE: usize = 64;

/// The 32,096 records; record 0 describes the container, 16,255 sprites have pieces.
#[derive(Debug)]
pub struct Sprites<'a> {
    pak: Pak<'a>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sprite<'a> {
    /// The sprite's size; pieces of 64-pixel blocks may run past it (62,406 of 209,956 do).
    pub width: u16,
    pub height: u16,
    /// `+0x08`: meaning unknown; openheilig found that adding it misplaces the sprite.
    pub offset: [i16; 2],
    pub pieces: Vec<Piece<'a>>,
}

/// Piece of a sprite: a rectangle of an atlas, as large there as in the sprite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece<'a> {
    /// The atlas name, `MIXnnn.444`.
    pub atlas: &'a [u8],
    /// The atlas: a `texture.pak` record.
    pub texture: u32,
    /// The piece in sprite pixels: `x0, y0, x1, y1`.
    pub rect: [u16; 4],
    /// The piece in the atlas as fractions of its size, at whole pixels: `u0, v0, u1, v1`.
    pub uv: [f32; 4],
}

impl<'a> Sprites<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        Ok(Self {
            pak: Pak::parse_expecting(data, MAGIC, VERSION)?,
        })
    }

    pub fn len(&self) -> usize {
        self.pak.entries().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Record `index` as stored.
    pub fn bytes(&self, index: usize) -> Result<&'a [u8]> {
        let entry = self.pak.entry(index)?;
        self.pak
            .record_reader(index, entry)
            .bytes(to_usize(entry.size), "sprite")
    }

    /// Sprite `index`, from 1: `u32` piece count, `u16` width, `u16` height, `i16[2]`, a fill
    /// word; then the 64-byte pieces.
    pub fn get(&self, index: usize) -> Result<Sprite<'a>> {
        let bytes = self.bytes(index)?;
        let mut r = Reader::new(bytes).with_record(index);
        if index == 0 {
            return Err(r.error(field(
                "sprite",
                0,
                "from 1; record 0 describes the file".into(),
            )));
        }
        let count_at = r.offset();
        let count = to_usize(r.u32()?);
        let expected = count.saturating_mul(PIECE_SIZE).saturating_add(HEADER_SIZE);
        if bytes.len() != expected {
            return Err(r.error_at(
                count_at,
                field(
                    "sprite size",
                    u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                    format!("{expected}, 16 + 64 per piece"),
                ),
            ));
        }
        let width = r.u16()?;
        let height = r.u16()?;
        let offset = [
            i16::from_le_bytes(r.array("sprite offset")?),
            i16::from_le_bytes(r.array("sprite offset")?),
        ];
        r.u32()?; // the fill 0xCCCCCCCC
        let mut pieces = Vec::with_capacity(count);
        for _ in 0..count {
            let name = r.bytes(32, "piece atlas")?;
            let texture = r.u32()?;
            let [x1, y1, x0, y0] = [r.u16()?, r.u16()?, r.u16()?, r.u16()?];
            r.u32()?; // 0
            let uv = r.f32s()?;
            pieces.push(Piece {
                atlas: name.split(|&b| b == 0).next().unwrap_or_default(),
                texture,
                rect: [x0, y0, x1, y1],
                uv,
            });
        }
        Ok(Sprite {
            width,
            height,
            offset,
            pieces,
        })
    }
}

#[cfg(test)]
mod game_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{PakRecord, build_pak};

    fn piece(texture: u32, rect: [u16; 4], uv: [f32; 4]) -> Vec<u8> {
        let mut p = b"MIX0.444".to_vec();
        p.resize(32, 0);
        p.extend_from_slice(&texture.to_le_bytes());
        let [x0, y0, x1, y1] = rect;
        for v in [x1, y1, x0, y0] {
            p.extend_from_slice(&v.to_le_bytes());
        }
        p.extend_from_slice(&0u32.to_le_bytes());
        for v in uv {
            p.extend_from_slice(&v.to_le_bytes());
        }
        p
    }

    fn sprites() -> Vec<u8> {
        let mut sprite = 2u32.to_le_bytes().to_vec();
        sprite.extend_from_slice(&105u16.to_le_bytes());
        sprite.extend_from_slice(&90u16.to_le_bytes());
        sprite.extend_from_slice(&0i16.to_le_bytes());
        sprite.extend_from_slice(&(-7i16).to_le_bytes());
        sprite.extend_from_slice(&0xCCCC_CCCCu32.to_le_bytes());
        sprite.extend(piece(7, [0, 0, 64, 64], [0.0, 0.0, 0.25, 0.25]));
        sprite.extend(piece(
            7,
            [64, 0, 107, 70],
            [0.25, 0.0, 0.418_75, 0.273_437_5],
        ));
        let header = [99u32, 0, 16, 0xCCCC_CCCC]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>();
        build_pak(
            MAGIC,
            VERSION,
            &[
                Some(PakRecord::new(99, &header)),
                Some(PakRecord::new(99, &sprite)),
            ],
        )
    }

    #[test]
    fn reads_a_sprite_and_its_pieces() {
        let data = sprites();
        let sprites = Sprites::parse(&data).unwrap();
        let sprite = sprites.get(1).unwrap();
        assert_eq!(
            (sprite.width, sprite.height, sprite.offset),
            (105, 90, [0, -7])
        );
        assert_eq!(sprite.pieces.len(), 2);
        let second = sprite.pieces[1];
        assert_eq!(second.atlas, b"MIX0.444");
        assert_eq!((second.texture, second.rect), (7, [64, 0, 107, 70]));
        assert_eq!(second.uv[2], 0.418_75);
        assert!(sprites.get(0).is_err());
    }

    #[test]
    fn the_size_must_hold_the_pieces() {
        let mut data = sprites();
        let at = data.len() - 2 * PIECE_SIZE - HEADER_SIZE;
        data[at] = 3;
        let err = Sprites::parse(&data).unwrap().get(1).unwrap_err();
        assert!(
            err.to_string()
                .ends_with("field `sprite size` is 144, expected 208, 16 + 64 per piece"),
            "{err}"
        );
    }
}
