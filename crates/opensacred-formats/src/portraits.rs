//! The creatures' portraits of the dialogues' window compiled into `Sacred.exe` (docs/re/
//! netscript.md, "Dialogues"): the function `0x4345C0`, which gives each creature type the index
//! of its image of the interface, a switch over two jump tables whose cases each load a constant
//! (`mov eax, imm32; ret 4`), its default too; and the interface's images' table at `0x9ECEB0`,
//! entries of 0x54 bytes: the index, its kind, the TGA's name, the image's rectangle in it and its
//! two anchors (docs/re/interface.md, "The images' table at run time"). Fixed virtual
//! addresses of the GOG 2.28 build, which the caller checks against the manifest.

use crate::error::{Result, field};
use crate::pe::Image;

/// The function's jump tables in GOG 2.28: the types 1 to 9's and the types `0x21` to `0x2CA`'s,
/// a case's address each.
const LOW_TABLE: u32 = 0x0043_5264;
const LOW_TYPES: (u32, u32) = (1, 9);
const HIGH_TABLE: u32 = 0x0043_5288;
const HIGH_TYPES: (u32, u32) = (0x21, 0x2CA);
/// The cases of the two types it compares alone, `0x20` and `0x17E2`.
const SINGLE_CASES: [(u32, u32); 2] = [(0x20, 0x0043_460C), (0x17E2, 0x0043_4BE2)];
/// The default case, which the types without one of their own take.
pub const DEFAULT_CASE: u32 = 0x0043_525A;
/// The interface's images' table and the size of an entry.
pub const IMAGES_ADDRESS: u32 = 0x009E_CEB0;
const IMAGE_SIZE: u32 = 0x54;
/// Where an entry keeps its kind, its file's name, its rectangle and its anchors.
const KIND_AT: usize = 4;
const NAME_AT: usize = 8;
const RECT_AT: usize = 0x2C;
const ANCHORS_AT: usize = 0x3C;

/// An image of the interface: its index, its TGA's name, its rectangle in it, `x0`, `y0`, `x1`,
/// `y1`, the last pixels included, its kind (`+0x04`, 9 for the buttons and icons, which take
/// their anchors at their centre), and its two anchor points `ax0`, `ay0`, `ax1`, `ay1` in the
/// atlas's pixels, 0 for none (`+0x3C` to `+0x48`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InterfaceImage {
    pub index: u32,
    pub file: String,
    pub rect: [f32; 4],
    pub kind: u32,
    pub anchors: [i32; 4],
}

/// The portraits' images by the function `0x4345C0`: the default's index, and each type's with a
/// case of its own, in the order of the types.
pub fn creature_images(exe: &[u8]) -> Result<(u32, Vec<(u32, u32)>)> {
    let image = Image::parse(exe)?;
    let mut cases = SINGLE_CASES.to_vec();
    for (table, (first, last)) in [(LOW_TABLE, LOW_TYPES), (HIGH_TABLE, HIGH_TYPES)] {
        let count = usize::try_from(last - first + 1).unwrap_or(0);
        let mut r = image.at(table, count * 4, "portraits' jump table")?;
        for creature in first..=last {
            cases.push((creature, r.u32()?));
        }
    }
    let default = constant(&image, DEFAULT_CASE)?;
    let mut images = Vec::new();
    for (creature, case) in cases {
        if case != DEFAULT_CASE {
            images.push((creature, constant(&image, case)?));
        }
    }
    images.sort_unstable();
    Ok((default, images))
}

/// The constant that the case at `case` returns: its code `mov eax, imm32; ret 4`.
fn constant(image: &Image<'_>, case: u32) -> Result<u32> {
    let mut r = image.at(case, 8, "portrait's case")?;
    let start = r.offset();
    let code = r.bytes(8, "portrait's case")?;
    let shape = code.first() == Some(&0xB8) && code.get(5..8) == Some(&[0xC2, 0x04, 0x00][..]);
    match (
        shape,
        code.get(1..5).and_then(|b| <[u8; 4]>::try_from(b).ok()),
    ) {
        (true, Some(value)) => Ok(u32::from_le_bytes(value)),
        _ => Err(r.error_at(
            start,
            field(
                "portrait's case",
                u64::from(case),
                "mov eax, imm32; ret 4".into(),
            ),
        )),
    }
}

/// The interface's images of the indices `indices`, from the table at `0x9ECEB0`.
pub fn interface_images(
    exe: &[u8],
    indices: impl IntoIterator<Item = u32>,
) -> Result<Vec<InterfaceImage>> {
    let image = Image::parse(exe)?;
    indices
        .into_iter()
        .map(|index| match entry(&image, index)? {
            Some(found) => Ok(found),
            None => {
                let mut r =
                    image.at(IMAGES_ADDRESS + index * IMAGE_SIZE, 4, "interface's image")?;
                let start = r.offset();
                let stored = r.u32()?;
                Err(r.error_at(
                    start,
                    field("interface's image index", stored.into(), index.to_string()),
                ))
            }
        })
        .collect()
}

/// The whole of the interface's images' table at `0x9ECEB0`, from its entry 0 (`INVALID`) to the
/// last whose stored index is its own (1,824 in GOG 2.28; #1087).
pub fn all_interface_images(exe: &[u8]) -> Result<Vec<InterfaceImage>> {
    let image = Image::parse(exe)?;
    let mut images = Vec::new();
    for index in 0..MOST_IMAGES {
        match entry(&image, index)? {
            Some(found) => images.push(found),
            None => break,
        }
    }
    Ok(images)
}

/// The most entries the table is read for, far past its end.
const MOST_IMAGES: u32 = 8_192;

/// The entry `index` of the images' table, none where its stored index is not its own.
fn entry(image: &Image<'_>, index: u32) -> Result<Option<InterfaceImage>> {
    let at = IMAGES_ADDRESS + index * IMAGE_SIZE;
    let mut r = image.at(at, 0x54, "interface's image")?;
    let entry = r.bytes(0x54, "interface's image")?;
    let stored = u32::from_le_bytes(entry[..4].try_into().unwrap_or_default());
    if stored != index {
        return Ok(None);
    }
    let name = &entry[NAME_AT..RECT_AT];
    let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
    let file = name[..end].iter().map(|&b| char::from(b)).collect();
    let mut rect = [0.0; 4];
    for (k, value) in rect.iter_mut().enumerate() {
        let at = RECT_AT + 4 * k;
        *value = f32::from_le_bytes(entry[at..at + 4].try_into().unwrap_or_default());
    }
    let word = |at: usize| entry[at..at + 4].try_into().unwrap_or_default();
    let kind = u32::from_le_bytes(word(KIND_AT));
    let anchors = std::array::from_fn(|k| i32::from_le_bytes(word(ANCHORS_AT + 4 * k)));
    Ok(Some(InterfaceImage {
        index,
        file,
        rect,
        kind,
        anchors,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pe::tests::image;

    /// Where the switch's test image ends, past its second jump table.
    const END: u32 = 0x0043_6000;

    /// A switch whose jump tables send the type 1 to a case of 964, the type `0x21` to the
    /// default and the rest to a case of 900, at the function's own addresses.
    fn switch() -> Vec<u8> {
        let base = 0x0043_45C0_u32;
        let mut code = vec![0_u8; usize::try_from(END - base).unwrap()];
        let put = |code: &mut Vec<u8>, at: u32, bytes: &[u8]| {
            let from = usize::try_from(at - base).unwrap();
            code[from..from + bytes.len()].copy_from_slice(bytes);
        };
        let case = |value: u32| {
            let mut bytes = vec![0xB8];
            bytes.extend(value.to_le_bytes());
            bytes.extend([0xC2, 0x04, 0x00]);
            bytes
        };
        put(&mut code, 0x0043_45DC, &case(964));
        put(&mut code, 0x0043_45E4, &case(900));
        put(&mut code, DEFAULT_CASE, &case(900));
        for (_, at) in SINGLE_CASES {
            put(&mut code, at, &case(931));
        }
        for i in 0..9 {
            let target: u32 = if i == 0 { 0x0043_45DC } else { 0x0043_45E4 };
            put(&mut code, LOW_TABLE + 4 * i, &target.to_le_bytes());
        }
        for i in 0..(HIGH_TYPES.1 - HIGH_TYPES.0 + 1) {
            let target = if i == 0 { DEFAULT_CASE } else { 0x0043_45E4 };
            put(&mut code, HIGH_TABLE + 4 * i, &target.to_le_bytes());
        }
        image(base, &code)
    }

    #[test]
    fn each_type_takes_its_cases_constant_or_the_defaults() {
        let (default, images) = creature_images(&switch()).unwrap();
        assert_eq!(default, 900);
        assert_eq!(images.first(), Some(&(1, 964)));
        assert!(images.contains(&(0x20, 931)) && images.contains(&(0x17E2, 931)));
        assert!(!images.iter().any(|&(creature, _)| creature == 0x21));
        assert_eq!(images.len(), 9 + 0x2A9 + 2);
    }

    #[test]
    fn a_case_of_another_shape_is_refused() {
        let mut exe = switch();
        let at = exe.len() - usize::try_from(END - 0x0043_45DC).unwrap();
        exe[at] = 0x90;
        assert!(creature_images(&exe).is_err());
    }

    #[test]
    fn an_image_gives_its_file_and_its_rectangle() {
        let mut table = vec![0_u8; usize::try_from(IMAGE_SIZE * 3).unwrap()];
        let entry = &mut table[0x54 * 2..0x54 * 3];
        entry[..4].copy_from_slice(&2_u32.to_le_bytes());
        entry[NAME_AT..NAME_AT + 19].copy_from_slice(b"GUI_portraitX34.TGA");
        for (k, v) in [96.0_f32, 0.0, 191.0, 127.0].into_iter().enumerate() {
            entry[RECT_AT + 4 * k..RECT_AT + 4 * k + 4].copy_from_slice(&v.to_le_bytes());
        }
        entry[KIND_AT..KIND_AT + 4].copy_from_slice(&9_u32.to_le_bytes());
        for (k, v) in [102_i32, 65, 0, 0].into_iter().enumerate() {
            entry[ANCHORS_AT + 4 * k..ANCHORS_AT + 4 * k + 4].copy_from_slice(&v.to_le_bytes());
        }
        let exe = image(IMAGES_ADDRESS, &table);
        let images = interface_images(&exe, [2]).unwrap();
        assert_eq!(
            images,
            [InterfaceImage {
                index: 2,
                file: "GUI_portraitX34.TGA".into(),
                rect: [96.0, 0.0, 191.0, 127.0],
                kind: 9,
                anchors: [102, 65, 0, 0],
            }]
        );
        assert!(interface_images(&exe, [1]).is_err());
    }
}
