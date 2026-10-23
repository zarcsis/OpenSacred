//! The byte table of the creatures' voices compiled into `Sacred.exe` (docs/re/sound.md, "The
//! fight's sounds"): for each creature type, 1 to 714, the case of the voice function `0x6852B0`
//! that speaks for it, `0x75` none. It lies at a fixed virtual address of the GOG 2.28 build,
//! which the caller checks against the manifest; what each case does is read off its code, not
//! from the data.

use crate::error::{Result, field};
use crate::pe::Image;

/// The byte table in GOG 2.28: the case of the type 1 first.
pub const CASES_ADDRESS: u32 = 0x0068_BBD0;
/// The types it covers, 1 to 714: the function gives no voice to a type above.
pub const TYPES: usize = 714;
/// The cases of the jump table at `0x68B9F8`.
pub const CASE_COUNT: u8 = 118;
/// The case of a type without a voice.
pub const NO_VOICE: u8 = 0x75;

/// The case of each type, the type 1's first.
pub fn cases(exe: &[u8]) -> Result<Vec<u8>> {
    let image = Image::parse(exe)?;
    let mut r = image.at(CASES_ADDRESS, TYPES, "table of voices")?;
    let start = r.offset();
    let cases = (0..TYPES).map(|_| r.u8()).collect::<Result<Vec<_>>>()?;
    if let Some(at) = cases.iter().position(|&case| case >= CASE_COUNT) {
        return Err(r.error_at(
            start + at,
            field(
                "voice case",
                u64::from(cases[at]),
                format!("below {CASE_COUNT}"),
            ),
        ));
    }
    Ok(cases)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pe::tests::image;

    #[test]
    fn every_type_has_its_case() {
        let mut table = vec![NO_VOICE; TYPES];
        table[0] = 0;
        table[75] = 0x2A;
        let cases = cases(&image(CASES_ADDRESS, &table)).unwrap();
        assert_eq!(cases.len(), TYPES);
        assert_eq!(cases[0], 0);
        assert_eq!(cases[75], 0x2A);
        assert_eq!(cases[713], NO_VOICE);
    }

    #[test]
    fn a_case_beyond_the_jump_table_is_refused() {
        let mut table = vec![NO_VOICE; TYPES];
        table[3] = CASE_COUNT;
        assert!(cases(&image(CASES_ADDRESS, &table)).is_err());
    }
}
