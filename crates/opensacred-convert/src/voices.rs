//! The creatures' voices (docs/re/sound.md, "The fight's sounds"): `Sacred.exe`'s byte table at
//! `0x68BBD0` gives each creature type the case of the voice function `0x6852B0` that speaks for
//! it, and [`CASES`] says what each of the 118 cases does, as read off the code and checked in the
//! running original (#710), → `tables/voices.ron` ([`opensacred_data::tables::Voices`]).

use anyhow::{Context, Result};
use opensacred_data::tables::{Voice, Voices};
use opensacred_formats::voices::{self, CASE_COUNT};

/// What a case of `0x6852B0` gives its types: the profile, or the heroes' two, the roll `n` of
/// each group (`ATTACK`, `NORMALHIT`, `CRITICALHIT`, `DEATH`; 1 always, 0 never), the wait of a
/// sound id that played, in seconds, and the fixed ids of the lines at a cast and when the
/// creature falls unconscious (the special 1 and 2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Case {
    pub profiles: &'static [u16],
    pub rolls: [u8; 4],
    pub wait: f32,
    pub casts: &'static [u16],
    pub faints: &'static [u16],
}

impl Case {
    /// A case without a voice: `0x75`, and four that name the profile 0.
    const NONE: Self = Self::new(&[], [0; 4], 0.0, &[], &[]);

    const fn new(
        profiles: &'static [u16],
        rolls: [u8; 4],
        wait: f32,
        casts: &'static [u16],
        faints: &'static [u16],
    ) -> Self {
        Self {
            profiles,
            rolls,
            wait,
            casts,
            faints,
        }
    }
}

/// The 118 cases of `0x6852B0`, by their index in the jump table at `0x68B9F8`, each with the
/// address of its code: the rolls are `rand() % n` (`rand() & 3` for 4, `& 1` for 2), the
/// `DEATH` forced to 0 in every case that speaks; a case's code ending the switch waits 10 s,
/// the jumps to `0x68A67D` and `0x68B613` 2 s and 5 s, the others 1 s or a time of their own.
/// Checked in the original with every type, every group and a controlled `rand()` (#710).
#[rustfmt::skip]
pub const CASES: [Case; CASE_COUNT as usize] = [
    Case::new(&[1008, 1009], [4, 4, 4, 1], 10.0, &[], &[]), // 0x00 at 0x689a94
    Case::new(&[1002, 1003], [4, 4, 4, 1], 10.0, &[], &[]), // 0x01 at 0x6899d2
    Case::new(&[1006, 1007], [4, 4, 4, 1], 10.0, &[], &[]), // 0x02 at 0x689a53
    Case::new(&[1000, 1001], [4, 4, 4, 1], 10.0, &[], &[]), // 0x03 at 0x689991
    Case::new(&[1004, 1005], [4, 4, 4, 1], 10.0, &[], &[]), // 0x04 at 0x689a12
    Case::new(&[1010, 1011], [4, 4, 4, 1], 10.0, &[], &[]), // 0x05 at 0x689ad4
    Case::new(&[1013, 1014], [4, 4, 4, 1], 10.0, &[], &[]), // 0x06 at 0x689f64
    Case::new(&[1015, 1016], [4, 4, 4, 1], 10.0, &[], &[]), // 0x07 at 0x689fa4
    Case::new(&[2035], [4, 4, 4, 1], 1.0, &[], &[]), // 0x08 at 0x68b36a
    Case::new(&[2017], [3, 3, 3, 1], 2.0, &[], &[]), // 0x09 at 0x68ad5b
    Case::new(&[2017], [3, 3, 3, 1], 20.0, &[], &[]), // 0x0a at 0x68ad75
    Case::new(&[2020], [2, 2, 2, 1], 4.0, &[], &[]), // 0x0b at 0x68ae0f
    Case::new(&[2029], [4, 4, 4, 1], 10.0, &[], &[]), // 0x0c at 0x68b00c
    Case::new(&[2034], [4, 4, 4, 1], 10.0, &[], &[]), // 0x0d at 0x68b073
    Case::new(&[2030], [4, 4, 4, 1], 10.0, &[], &[]), // 0x0e at 0x68b10e
    Case::new(&[2045], [4, 4, 4, 1], 10.0, &[], &[]), // 0x0f at 0x68b0a7
    Case::new(&[2027], [2, 2, 2, 1], 4.0, &[], &[]), // 0x10 at 0x68afa5
    Case::new(&[2018], [2, 2, 2, 1], 2.0, &[], &[]), // 0x11 at 0x68ae3e
    Case::new(&[2039], [3, 3, 3, 1], 1.0, &[], &[]), // 0x12 at 0x68b2d8
    Case::new(&[2038], [3, 3, 3, 1], 1.0, &[], &[]), // 0x13 at 0x68b307
    Case::new(&[2037], [4, 4, 4, 1], 1.0, &[], &[]), // 0x14 at 0x68b336
    Case::new(&[2036], [4, 4, 4, 1], 1.0, &[], &[]), // 0x15 at 0x68b39d
    Case::new(&[2040], [2, 2, 2, 1], 1.0, &[], &[]), // 0x16 at 0x68b3d1
    Case::new(&[2047], [2, 2, 2, 1], 5.0, &[11409, 11410, 11411, 11412], &[]), // 0x17 at 0x68b544
    Case::new(&[2028], [4, 4, 4, 1], 1.0, &[], &[]), // 0x18 at 0x68afd8
    Case::new(&[2019], [4, 4, 4, 1], 2.0, &[], &[]), // 0x19 at 0x68b2a4
    Case::new(&[2044], [1, 1, 1, 1], 1.0, &[], &[]), // 0x1a at 0x68b5db
    Case::new(&[2046], [2, 2, 2, 1], 5.0, &[11381, 11382], &[]), // 0x1b at 0x68b5e5
    Case::new(&[2043], [4, 4, 4, 1], 1.0, &[], &[]), // 0x1c at 0x68b667
    Case::new(&[2031], [4, 4, 4, 1], 5.0, &[], &[]), // 0x1d at 0x68b69b
    Case::new(&[2054], [2, 2, 2, 1], 10.0, &[], &[19508]), // 0x1e at 0x68a828
    Case::new(&[2116], [2, 2, 2, 1], 10.0, &[], &[20207]), // 0x1f at 0x68a6e9
    Case::new(&[2046], [4, 4, 4, 1], 10.0, &[], &[]), // 0x20 at 0x68b0db
    Case::new(&[2067], [4, 4, 4, 1], 10.0, &[], &[]), // 0x21 at 0x68b040
    Case::new(&[2059], [4, 4, 4, 1], 10.0, &[], &[]), // 0x22 at 0x68a9cf
    Case::new(&[2060], [4, 4, 4, 1], 10.0, &[], &[]), // 0x23 at 0x68b6cf
    Case::new(&[2066], [2, 2, 2, 1], 10.0, &[], &[]), // 0x24 at 0x68aa9a
    Case::new(&[2062], [4, 4, 4, 1], 10.0, &[], &[]), // 0x25 at 0x68b17a
    Case::new(&[2017], [3, 3, 3, 1], 2.0, &[], &[]), // 0x26 at 0x68ad9f
    Case::new(&[2016], [3, 3, 3, 1], 20.0, &[], &[]), // 0x27 at 0x68add7
    Case::new(&[2066], [2, 2, 2, 1], 10.0, &[], &[]), // 0x28 at 0x68aa07
    Case::new(&[2066], [2, 2, 2, 1], 10.0, &[], &[]), // 0x29 at 0x68aa51
    Case::new(&[2023], [7, 5, 5, 1], 10.0, &[], &[]), // 0x2a at 0x68ad09
    Case::new(&[2106], [3, 5, 5, 1], 10.0, &[], &[]), // 0x2b at 0x68ac29
    Case::new(&[2107], [3, 5, 5, 1], 10.0, &[], &[]), // 0x2c at 0x68ac61
    Case::new(&[2103], [3, 5, 5, 1], 10.0, &[], &[]), // 0x2d at 0x68ab49
    Case::new(&[2104], [3, 5, 5, 1], 10.0, &[], &[]), // 0x2e at 0x68ab81
    Case::new(&[2016], [3, 3, 3, 1], 2.0, &[], &[]), // 0x2f at 0x68a692
    Case::new(&[2129], [2, 2, 2, 1], 5.0, &[20633, 20634, 20635], &[]), // 0x30 at 0x68a3c2
    Case::new(&[2018], [2, 2, 2, 1], 5.0, &[20618, 20619, 20620], &[]), // 0x31 at 0x68a5b7
    Case::new(&[2125], [2, 2, 2, 1], 5.0, &[], &[]), // 0x32 at 0x68a35b
    Case::new(&[2139], [2, 2, 2, 1], 3.0, &[20812, 20813], &[]), // 0x33 at 0x689fe5
    Case::new(&[2127], [2, 2, 2, 1], 2.0, &[], &[]), // 0x34 at 0x68a6bb
    Case::new(&[2146], [2, 2, 2, 1], 2.0, &[], &[]), // 0x35 at 0x68a656
    Case::new(&[2122], [2, 2, 2, 1], 5.0, &[], &[]), // 0x36 at 0x68a550
    Case::new(&[2120], [2, 2, 2, 1], 5.0, &[], &[]), // 0x37 at 0x68a4ab
    Case::new(&[2124], [2, 2, 2, 1], 5.0, &[], &[]), // 0x38 at 0x68a461
    Case::new(&[2144], [1, 1, 1, 1], 2.0, &[], &[]), // 0x39 at 0x68a528
    Case::new(&[2018], [2, 2, 2, 1], 5.0, &[], &[]), // 0x3a at 0x68a2f7
    Case::new(&[2123], [2, 2, 2, 1], 5.0, &[], &[]), // 0x3b at 0x68a4f4
    Case::new(&[2119], [2, 2, 2, 1], 5.0, &[], &[]), // 0x3c at 0x68a38f
    Case::new(&[2121], [1, 1, 1, 1], 5.0, &[], &[]), // 0x3d at 0x68a341
    Case::new(&[2120], [2, 2, 2, 1], 5.0, &[], &[]), // 0x3e at 0x68a584
    Case::new(&[2046], [4, 4, 4, 1], 3.0, &[], &[]), // 0x3f at 0x68a2c4
    Case::new(&[2126], [2, 2, 2, 1], 3.0, &[], &[]), // 0x40 at 0x68a25c
    Case::new(&[2121], [2, 2, 2, 1], 1.0, &[], &[]), // 0x41 at 0x689e21
    Case::new(&[2143], [2, 2, 2, 1], 3.0, &[], &[]), // 0x42 at 0x689ded
    Case::new(&[2145], [2, 2, 2, 1], 3.0, &[], &[]), // 0x43 at 0x689dba
    Case::new(&[2027], [2, 2, 2, 1], 1.5, &[], &[]), // 0x44 at 0x689c96
    Case::new(&[2098], [4, 4, 4, 1], 3.0, &[], &[]), // 0x45 at 0x68a290
    Case::new(&[2063], [4, 4, 4, 1], 10.0, &[], &[]), // 0x46 at 0x68b1b1
    Case::new(&[2064], [4, 4, 4, 1], 10.0, &[], &[]), // 0x47 at 0x68b1e9
    Case::new(&[2053], [4, 4, 4, 1], 10.0, &[], &[]), // 0x48 at 0x68b702
    Case::NONE, // 0x49 at 0x689f09
    Case::NONE, // 0x4a at 0x689ee3
    Case::new(&[2141], [2, 2, 2, 1], 3.0, &[], &[20831, 20832, 20833, 20834]), // 0x4b at 0x689e5d
    Case::new(&[2033], [4, 4, 4, 1], 10.0, &[], &[]), // 0x4c at 0x68b736
    Case::new(&[2133], [2, 2, 2, 1], 3.0, &[20680, 20681], &[]), // 0x4d at 0x68a0ae
    Case::new(&[2136], [2, 2, 2, 1], 3.0, &[20748, 20749], &[]), // 0x4e at 0x68a177
    Case::new(&[2131], [2, 2, 2, 1], 3.0, &[20658, 20659], &[]), // 0x4f at 0x68a1ea
    Case::new(&[2110], [4, 4, 4, 1], 10.0, &[], &[]), // 0x50 at 0x68ae5c
    Case::new(&[2108], [3, 5, 5, 1], 10.0, &[], &[]), // 0x51 at 0x68ac99
    Case::new(&[2065], [4, 4, 4, 1], 10.0, &[], &[]), // 0x52 at 0x68b21a
    Case::new(&[2033], [4, 4, 4, 1], 10.0, &[], &[]), // 0x53 at 0x68b24a
    Case::new(&[2111], [4, 4, 4, 1], 10.0, &[], &[]), // 0x54 at 0x68ae7b
    Case::new(&[2022], [4, 4, 4, 1], 10.0, &[], &[]), // 0x55 at 0x68ae9a
    Case::new(&[2112], [4, 4, 4, 1], 10.0, &[], &[]), // 0x56 at 0x68aeb8
    Case::new(&[2113], [4, 4, 4, 1], 10.0, &[], &[]), // 0x57 at 0x68aed7
    Case::new(&[2113], [4, 4, 4, 1], 10.0, &[], &[]), // 0x58 at 0x68aef6
    Case::new(&[2113], [4, 4, 4, 1], 10.0, &[], &[]), // 0x59 at 0x68af49
    Case::new(&[2025], [4, 4, 4, 1], 10.0, &[], &[]), // 0x5a at 0x68af67
    Case::new(&[2105], [3, 5, 5, 1], 10.0, &[], &[]), // 0x5b at 0x68abb9
    Case::new(&[2032], [3, 5, 5, 1], 10.0, &[], &[]), // 0x5c at 0x68abf1
    Case::new(&[2109], [3, 5, 5, 1], 10.0, &[], &[]), // 0x5d at 0x68acd1
    Case::new(&[2024], [6, 6, 6, 1], 10.0, &[], &[]), // 0x5e at 0x68ad41
    Case::new(&[2061], [2, 2, 2, 1], 10.0, &[], &[]), // 0x5f at 0x68a8bb
    Case::new(&[2098], [2, 2, 2, 1], 10.0, &[], &[]), // 0x60 at 0x68aace
    Case::new(&[2102], [4, 4, 4, 1], 10.0, &[], &[]), // 0x61 at 0x68b142
    Case::new(&[2118], [2, 2, 2, 1], 10.0, &[], &[20179]), // 0x62 at 0x68a7de
    Case::new(&[2057], [2, 2, 2, 1], 10.0, &[], &[19538]), // 0x63 at 0x68a872
    Case::new(&[2053], [4, 4, 4, 1], 10.0, &[], &[19480, 19481, 19482, 19483]), // 0x64 at 0x68a939
    Case::new(&[2130], [2, 2, 2, 1], 3.0, &[], &[]), // 0x65 at 0x689f30
    Case::new(&[2134], [2, 2, 2, 1], 3.0, &[], &[20689, 20690, 20691, 20692]), // 0x66 at 0x689b29
    Case::NONE, // 0x67 at 0x689c36
    Case::new(&[2140], [2, 2, 2, 1], 3.0, &[], &[]), // 0x68 at 0x689d86
    Case::new(&[2027], [2, 2, 2, 1], 3.0, &[], &[]), // 0x69 at 0x689ce0
    Case::NONE, // 0x6a at 0x689c64
    Case::new(&[2135], [2, 2, 2, 1], 3.0, &[20735, 20736], &[]), // 0x6b at 0x689d13
    Case::new(&[2138], [2, 2, 2, 1], 3.0, &[], &[20766, 20767, 20768, 20769]), // 0x6c at 0x689bbe
    Case::new(&[2014], [2, 2, 2, 1], 1.0, &[], &[]), // 0x6d at 0x68ab02
    Case::new(&[2009], [2, 2, 2, 1], 1.0, &[], &[]), // 0x6e at 0x68ab20
    Case::new(&[2004], [1, 1, 1, 1], 1.0, &[], &[]), // 0x6f at 0x68ab3f
    Case::new(&[2055], [4, 4, 4, 1], 10.0, &[], &[19518]), // 0x70 at 0x68b760
    Case::new(&[2026], [4, 4, 4, 1], 10.0, &[], &[]), // 0x71 at 0x68af86
    Case::new(&[2114], [4, 4, 4, 1], 5.0, &[19547, 19548, 19549, 19550], &[20170]), // 0x72 at 0x68b405
    Case::new(&[2117], [4, 4, 4, 1], 10.0, &[], &[20189]), // 0x73 at 0x68a8ef
    Case::new(&[2052], [4, 4, 4, 1], 5.0, &[19459, 19460, 19461, 19462], &[]), // 0x74 at 0x68b4ad
    Case::NONE, // 0x75 at 0x68b7a5
];

/// The voices of the types 1 to 714 by the byte table of `Sacred.exe`, which must be the
/// manifest's: it is read at the address of that build. A type of a case without a voice has
/// none.
pub fn voices(exe: &[u8]) -> Result<Voices> {
    let cases = voices::cases(exe).context("Sacred.exe: the voices' cases")?;
    let mut voices = Vec::new();
    for (creature, &case) in (1u16..).zip(&cases) {
        let case = CASES[usize::from(case)];
        if case.profiles.is_empty() {
            continue;
        }
        voices.push(Voice {
            creature,
            profiles: case.profiles.to_vec(),
            rolls: case.rolls,
            wait: case.wait,
            casts: case.casts.to_vec(),
            faints: case.faints.to_vec(),
        });
    }
    Ok(Voices { voices })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cases_without_a_voice_are_the_profile_0s_and_0x75() {
        let silent: Vec<_> = (0u8..)
            .zip(CASES)
            .filter(|(_, case)| case.profiles.is_empty())
            .map(|(index, _)| index)
            .collect();
        assert_eq!(silent, [0x49, 0x4A, 0x67, 0x6A, voices::NO_VOICE]);
        // Only the heroes' cases, 0 to 7, take two profiles.
        assert!(CASES[..8].iter().all(|case| case.profiles.len() == 2));
        assert!(CASES[8..].iter().all(|case| case.profiles.len() < 2));
        // A death is forced in every case that speaks.
        assert!(
            CASES
                .iter()
                .all(|case| case.profiles.is_empty() || case.rolls[3] == 1)
        );
    }
}
