//! The creatures' idle noises (docs/re/sound.md, "The idle noises", #714): what each creature
//! type makes in the sound manager's update (`0x69B770`), which follows the creatures near the
//! hero in two lists by the sound events 6 and 11 (`0x67A7A0`): those hostile to the hero, at a
//! roll of one in 20 a tick while they are at rest, and the others, at one in 70 → `tables/
//! idle_noises.ron` ([`opensacred_data::tables::IdleNoises`]). A type then makes its noise at its
//! own roll, a variant of its profile's group 0 or one of a run of sound ids, the id waiting the
//! noise's seconds after it played, a little higher or lower than the voices.
//!
//! [`HOSTILE`] and [`PEACEFUL`] say which types make which noise, as read off the update's two
//! switches by the type and checked in the running original (#714): the types left out make none.

use opensacred_data::tables::{IdleNoise, IdleNoises, IdleSound};

/// A noise: its roll (`rand() % n`, 1 always), what it plays, its wait in seconds and what it
/// adds to the voices' factor of the rate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Noise {
    pub roll: u8,
    pub sound: IdleSound,
    pub wait: f32,
    pub rate: f32,
}

impl Noise {
    const fn new(roll: u8, sound: IdleSound, wait: f32, rate: f32) -> Self {
        Self {
            roll,
            sound,
            wait,
            rate,
        }
    }
}

/// The types, in ranges, that make each noise.
type Table = [(&'static [(u16, u16)], Noise)];

/// The noises of the types hostile to the hero (the update's first list, `0x69CCC5`: the byte
/// table at `0x6A03C8` and the jump table at `0x6A0240`). A profile's group is the record's
/// state's, 0 at rest; the roll fails into the profile 0, which picks nothing.
#[rustfmt::skip]
pub const HOSTILE: &Table = &[
    (&[(32, 32), (48, 48), (80, 81), (112, 117), (355, 355), (708, 710), (713, 714)], Noise::new(2, IdleSound::Profile(2035), 10.0, 0.0)),
    (&[(33, 33), (178, 181), (204, 204), (350, 350), (711, 712)], Noise::new(3, IdleSound::Profile(2016), 2.0, 0.0)),
    (&[(34, 34), (119, 123)], Noise::new(3, IdleSound::Profile(2017), 2.0, 0.0)),
    (&[(35, 35), (54, 54), (57, 59), (96, 98)], Noise::new(2, IdleSound::Profile(2028), 2.0, 0.0)),
    (&[(36, 36), (50, 50), (73, 73), (124, 124)], Noise::new(4, IdleSound::Profile(2020), 2.0, 0.0)),
    (&[(37, 37), (84, 84)], Noise::new(2, IdleSound::Profile(2029), 30.0, 0.0)),
    (&[(38, 38)], Noise::new(2, IdleSound::Profile(2034), 30.0, 0.0)),
    (&[(39, 39)], Noise::new(2, IdleSound::Profile(2030), 30.0, 0.0)),
    (&[(40, 40)], Noise::new(2, IdleSound::Profile(2045), 30.0, 0.0)),
    (&[(41, 43)], Noise::new(3, IdleSound::Profile(2027), 5.0, 0.0)),
    (&[(44, 44), (64, 66)], Noise::new(4, IdleSound::Profile(2018), 2.0, 0.0)),
    (&[(45, 45), (60, 62), (68, 68), (89, 92), (202, 202), (344, 344), (359, 359)], Noise::new(2, IdleSound::Profile(2039), 6.0, 0.0)),
    (&[(46, 46), (63, 63), (125, 126), (130, 130), (597, 598), (706, 706)], Noise::new(2, IdleSound::Profile(2038), 6.0, 0.0)),
    (&[(47, 47), (70, 70), (93, 93), (95, 95), (106, 107)], Noise::new(2, IdleSound::Profile(2037), 5.0, 0.0)),
    (&[(49, 49), (51, 51), (137, 137), (360, 360)], Noise::new(2, IdleSound::Profile(2036), 10.0, 0.0)),
    (&[(52, 52), (74, 74), (88, 88), (100, 103)], Noise::new(2, IdleSound::Profile(2040), 2.0, 0.0)),
    (&[(53, 53)], Noise::new(1, IdleSound::Profile(2047), 2.0, 0.0)),
    (&[(55, 55), (237, 237)], Noise::new(4, IdleSound::Profile(2019), 0.0, 0.0)),
    (&[(56, 56)], Noise::new(2, IdleSound::Profile(2044), 2.0, 0.0)),
    (&[(67, 67)], Noise::new(2, IdleSound::Profile(2046), 2.0, 0.0)),
    (&[(71, 71), (75, 75), (99, 99), (109, 109), (234, 236)], Noise::new(4, IdleSound::Profile(2043), 0.0, 0.0)),
    (&[(76, 76), (118, 118), (127, 129)], Noise::new(3, IdleSound::Profile(2031), 30.0, 0.0)),
    (&[(77, 77)], Noise::new(2, IdleSound::Profile(2054), 0.0, 0.0)),
    (&[(85, 86)], Noise::new(2, IdleSound::Profile(2046), 30.0, 0.0)),
    (&[(87, 87)], Noise::new(2, IdleSound::Profile(2067), 30.0, 0.0)),
    (&[(105, 105), (108, 108), (134, 134), (321, 321)], Noise::new(2, IdleSound::Profile(2059), 20.0, 0.0)),
    (&[(133, 133), (140, 140), (146, 146), (326, 327)], Noise::new(2, IdleSound::Profile(2060), 20.0, 0.0)),
    (&[(136, 136)], Noise::new(2, IdleSound::Profile(2066), 0.0, 0.0)),
    (&[(141, 141), (258, 258), (262, 262)], Noise::new(2, IdleSound::Profile(2062), 20.0, 0.0)),
    (&[(142, 143)], Noise::new(3, IdleSound::Profile(2016), 2.0, 0.4)),
    (&[(144, 144)], Noise::new(2, IdleSound::Profile(2066), 0.0, -0.2)),
    (&[(145, 145)], Noise::new(2, IdleSound::Profile(2066), 0.0, 0.7)),
    (&[(164, 166)], Noise::new(4, IdleSound::Ids { first: 7015, count: 2 }, 0.0, 0.0)),
    (&[(171, 172), (214, 214), (313, 313)], Noise::new(3, IdleSound::Profile(2023), 60.0, 0.0)),
    (&[(173, 174), (215, 215), (272, 272)], Noise::new(3, IdleSound::Profile(2106), 60.0, 0.0)),
    (&[(175, 175), (273, 273)], Noise::new(3, IdleSound::Profile(2107), 60.0, 0.0)),
    (&[(176, 176), (259, 259)], Noise::new(3, IdleSound::Profile(2103), 60.0, 0.0)),
    (&[(177, 177), (304, 304)], Noise::new(3, IdleSound::Profile(2104), 60.0, 0.0)),
    (&[(182, 182)], Noise::new(4, IdleSound::Profile(2129), 2.0, 0.0)),
    (&[(183, 183)], Noise::new(4, IdleSound::Profile(2128), 2.0, 0.0)),
    (&[(184, 184), (249, 249)], Noise::new(4, IdleSound::Profile(2125), 2.0, 0.0)),
    (&[(185, 185)], Noise::new(4, IdleSound::Profile(2139), 2.0, 0.0)),
    (&[(186, 186), (333, 333)], Noise::new(2, IdleSound::Profile(2127), 2.0, 0.0)),
    (&[(187, 187), (205, 208), (253, 254), (330, 332)], Noise::new(2, IdleSound::Profile(2146), 1.5, 0.0)),
    (&[(188, 188), (245, 245)], Noise::new(4, IdleSound::Profile(2122), 2.0, 0.0)),
    (&[(189, 189), (334, 334)], Noise::new(4, IdleSound::Profile(2120), 2.0, -0.2)),
    (&[(190, 190), (335, 336)], Noise::new(1, IdleSound::Profile(2124), 1.0, -0.1)),
    (&[(191, 191), (353, 354), (358, 358)], Noise::new(4, IdleSound::Profile(2144), 2.0, 0.5)),
    (&[(192, 192), (329, 329), (342, 343)], Noise::new(4, IdleSound::Profile(2018), 2.0, 0.7)),
    (&[(193, 193)], Noise::new(1, IdleSound::Profile(2123), 1.0, 0.0)),
    (&[(194, 194), (246, 246)], Noise::new(2, IdleSound::Profile(2119), 2.0, 0.0)),
    (&[(195, 195), (198, 198), (247, 248), (345, 345)], Noise::new(1, IdleSound::Profile(2121), 0.0, 0.0)),
    (&[(196, 196), (337, 338)], Noise::new(2, IdleSound::Profile(2120), 2.0, 0.0)),
    (&[(197, 197), (339, 340)], Noise::new(4, IdleSound::Profile(2046), 2.0, 0.0)),
    (&[(199, 201)], Noise::new(2, IdleSound::Profile(2126), 2.0, 0.0)),
    (&[(203, 203), (351, 352)], Noise::new(1, IdleSound::Profile(2121), 0.5, 0.0)),
    (&[(209, 209), (347, 348)], Noise::new(1, IdleSound::Profile(2143), 3.5, 0.0)),
    (&[(210, 210), (346, 346)], Noise::new(1, IdleSound::Profile(2145), 3.5, 0.0)),
    (&[(211, 212), (349, 349)], Noise::new(2, IdleSound::Profile(2027), 2.5, 0.7)),
    (&[(213, 213), (341, 341)], Noise::new(4, IdleSound::Profile(2098), 2.0, 0.0)),
    (&[(216, 216), (263, 264)], Noise::new(2, IdleSound::Profile(2063), 20.0, 0.0)),
    (&[(217, 217), (265, 266)], Noise::new(2, IdleSound::Profile(2064), 20.0, 0.0)),
    (&[(218, 219), (693, 693)], Noise::new(2, IdleSound::Profile(2053), 20.0, 0.0)),
    (&[(224, 224), (356, 357)], Noise::new(4, IdleSound::Profile(2130), 2.0, 0.0)),
    (&[(238, 238), (269, 271), (319, 320)], Noise::new(2, IdleSound::Profile(2033), 20.0, 0.0)),
    (&[(250, 250)], Noise::new(4, IdleSound::Profile(2133), 2.0, 0.0)),
    (&[(251, 251)], Noise::new(4, IdleSound::Profile(2136), 2.0, 0.0)),
    (&[(252, 252)], Noise::new(4, IdleSound::Profile(2131), 2.0, 0.0)),
    (&[(257, 257), (274, 277)], Noise::new(5, IdleSound::Profile(2110), 60.0, 0.0)),
    (&[(260, 260)], Noise::new(3, IdleSound::Profile(2108), 60.0, 0.0)),
    (&[(267, 268)], Noise::new(2, IdleSound::Profile(2065), 20.0, 0.0)),
    (&[(278, 282)], Noise::new(5, IdleSound::Profile(2111), 60.0, 0.0)),
    (&[(283, 288)], Noise::new(5, IdleSound::Profile(2022), 60.0, 0.0)),
    (&[(289, 293)], Noise::new(5, IdleSound::Profile(2112), 60.0, 0.0)),
    (&[(294, 298)], Noise::new(5, IdleSound::Profile(2113), 60.0, 0.0)),
    (&[(299, 303)], Noise::new(5, IdleSound::Profile(2025), 60.0, 0.0)),
    (&[(305, 305)], Noise::new(3, IdleSound::Profile(2105), 60.0, 0.0)),
    (&[(306, 306), (314, 314)], Noise::new(3, IdleSound::Profile(2032), 60.0, 0.0)),
    (&[(307, 307)], Noise::new(3, IdleSound::Profile(2109), 60.0, 0.0)),
    (&[(308, 308)], Noise::new(3, IdleSound::Profile(2024), 60.0, 0.0)),
    (&[(311, 311)], Noise::new(2, IdleSound::Profile(2098), 0.0, 0.0)),
    (&[(312, 312)], Noise::new(2, IdleSound::Profile(2102), 20.0, 0.0)),
    (&[(323, 323)], Noise::new(2, IdleSound::Profile(2057), 0.0, 0.0)),
    (&[(361, 361)], Noise::new(4, IdleSound::Profile(2134), 2.0, 0.0)),
    (&[(365, 365)], Noise::new(2, IdleSound::Profile(2027), 2.0, 0.0)),
    (&[(366, 366)], Noise::new(4, IdleSound::Profile(2132), 2.0, 0.0)),
    (&[(367, 367)], Noise::new(4, IdleSound::Profile(2135), 2.0, 0.0)),
    (&[(368, 368)], Noise::new(4, IdleSound::Profile(2138), 2.0, 0.0)),
    (&[(512, 512), (567, 571), (573, 588), (599, 599)], Noise::new(2, IdleSound::Profile(2014), 0.0, 0.0)),
    (&[(536, 538), (591, 591), (600, 600)], Noise::new(3, IdleSound::Profile(2009), 0.0, 0.0)),
    (&[(543, 543), (545, 545)], Noise::new(3, IdleSound::Profile(2004), 0.0, 0.0)),
    (&[(689, 690)], Noise::new(5, IdleSound::Profile(2026), 5.0, 0.0)),
    (&[(700, 700)], Noise::new(2, IdleSound::Profile(2052), 2.0, 0.0)),
    (&[(707, 707)], Noise::new(4, IdleSound::Profile(1015), 2.0, 0.0)),
];

/// The noises of the types not hostile to the hero (the update's second list, `0x69ECF2`), each
/// of the group 0. The townsfolk's murmurs (the types 220 to 303 and 640 to 687 of runs of ids)
/// also wait for the record's flag 2 (`0x67A7A0`'s sub-command 4) to be clear; the types 702 to 705
/// play the run of the sound manager's `+0x8C` other than 1, and 52, 74, 88 and 100 to 103 the
/// group 0 where the update asks `0x678DA0` for one.
#[rustfmt::skip]
pub const PEACEFUL: &Table = &[
    (&[(47, 47), (70, 70), (93, 93), (95, 95), (106, 107)], Noise::new(1, IdleSound::Profile(2037), 5.0, 0.0)),
    (&[(52, 52), (74, 74), (88, 88), (100, 103)], Noise::new(1, IdleSound::Profile(2040), 2.0, 0.0)),
    (&[(72, 72)], Noise::new(1, IdleSound::Profile(105), 0.0, 0.0)),
    (&[(164, 166)], Noise::new(1, IdleSound::Ids { first: 7015, count: 2 }, 5.0, 0.0)),
    (&[(182, 182)], Noise::new(1, IdleSound::Profile(2129), 5.0, 0.0)),
    (&[(183, 183)], Noise::new(1, IdleSound::Profile(2128), 5.0, 0.0)),
    (&[(184, 184), (249, 249)], Noise::new(1, IdleSound::Profile(2125), 5.0, 0.0)),
    (&[(185, 185)], Noise::new(1, IdleSound::Profile(2139), 5.0, 0.0)),
    (&[(186, 186), (333, 333)], Noise::new(1, IdleSound::Profile(2127), 2.0, 0.0)),
    (&[(187, 187), (205, 208), (253, 254), (330, 332)], Noise::new(1, IdleSound::Profile(2146), 2.0, 0.0)),
    (&[(188, 188), (245, 245)], Noise::new(1, IdleSound::Profile(2122), 5.0, 0.0)),
    (&[(189, 189), (334, 334)], Noise::new(1, IdleSound::Profile(2120), 5.0, -0.2)),
    (&[(190, 190), (335, 336)], Noise::new(1, IdleSound::Profile(2124), 1.0, -0.1)),
    (&[(191, 191), (353, 354), (358, 358)], Noise::new(1, IdleSound::Profile(2144), 2.0, 0.5)),
    (&[(192, 192), (329, 329), (342, 343)], Noise::new(1, IdleSound::Profile(2018), 5.0, 0.7)),
    (&[(193, 193)], Noise::new(1, IdleSound::Profile(2123), 2.0, 0.0)),
    (&[(194, 194), (246, 246)], Noise::new(1, IdleSound::Profile(2119), 5.0, 0.0)),
    (&[(195, 195), (198, 198), (247, 248), (345, 345)], Noise::new(1, IdleSound::Profile(2121), 2.0, 0.0)),
    (&[(196, 196), (337, 338)], Noise::new(1, IdleSound::Profile(2120), 5.0, 0.0)),
    (&[(197, 197), (339, 340)], Noise::new(1, IdleSound::Profile(2046), 5.0, 0.7)),
    (&[(199, 201)], Noise::new(1, IdleSound::Profile(2126), 4.0, 0.7)),
    (&[(213, 213), (341, 341)], Noise::new(1, IdleSound::Profile(2098), 5.0, 0.0)),
    (&[(220, 221), (232, 232), (239, 244), (640, 644), (652, 656), (665, 671), (680, 684)], Noise::new(2, IdleSound::Ids { first: 13600, count: 58 }, 10.0, 0.0)),
    (&[(224, 224)], Noise::new(1, IdleSound::Profile(2141), 4.0, 0.0)),
    (&[(225, 226)], Noise::new(1, IdleSound::Ids { first: 11710, count: 22 }, 2.0, 0.0)),
    (&[(227, 228)], Noise::new(1, IdleSound::Ids { first: 11732, count: 4 }, 2.0, 0.0)),
    (&[(229, 230)], Noise::new(1, IdleSound::Ids { first: 11703, count: 7 }, 2.0, 0.0)),
    (&[(231, 231)], Noise::new(1, IdleSound::Ids { first: 11700, count: 3 }, 0.0, 0.0)),
    (&[(233, 233), (645, 651), (657, 663), (672, 676), (685, 687)], Noise::new(2, IdleSound::Ids { first: 13450, count: 47 }, 10.0, 0.0)),
    (&[(250, 250)], Noise::new(1, IdleSound::Profile(2133), 5.0, 0.0)),
    (&[(251, 251)], Noise::new(1, IdleSound::Profile(2136), 5.0, 0.0)),
    (&[(252, 252)], Noise::new(1, IdleSound::Profile(2131), 5.0, 0.0)),
    (&[(257, 257), (274, 303)], Noise::new(2, IdleSound::Ids { first: 16050, count: 29 }, 10.0, 0.0)),
    (&[(269, 271)], Noise::new(1, IdleSound::Profile(2033), 5.0, 0.0)),
    (&[(356, 357)], Noise::new(1, IdleSound::Profile(2130), 4.0, 0.0)),
    (&[(361, 361)], Noise::new(1, IdleSound::Profile(2134), 4.0, 0.0)),
    (&[(363, 363)], Noise::new(1, IdleSound::Profile(2140), 4.0, 0.0)),
    (&[(365, 365)], Noise::new(1, IdleSound::Profile(2027), 4.0, 0.0)),
    (&[(366, 366)], Noise::new(1, IdleSound::Profile(2132), 4.0, 0.0)),
    (&[(367, 367)], Noise::new(1, IdleSound::Profile(2135), 4.0, 0.0)),
    (&[(368, 368)], Noise::new(1, IdleSound::Profile(2138), 4.0, 0.0)),
    (&[(512, 512), (567, 571), (573, 588), (599, 599)], Noise::new(2, IdleSound::Profile(2014), 0.0, 0.0)),
    (&[(513, 513), (589, 590)], Noise::new(2, IdleSound::Profile(2006), 0.0, 0.0)),
    (&[(514, 515), (594, 596)], Noise::new(1, IdleSound::Profile(2002), 0.0, 0.0)),
    (&[(520, 522)], Noise::new(1, IdleSound::Profile(2015), 0.0, 0.0)),
    (&[(523, 528)], Noise::new(2, IdleSound::Profile(2005), 0.0, 0.0)),
    (&[(540, 541)], Noise::new(1, IdleSound::Profile(2021), 0.0, 0.0)),
    (&[(544, 544)], Noise::new(1, IdleSound::Profile(2013), 0.0, 0.0)),
    (&[(550, 558)], Noise::new(3, IdleSound::Profile(105), 0.0, 0.0)),
    (&[(560, 563), (565, 566)], Noise::new(2, IdleSound::Profile(2011), 0.0, 0.0)),
    (&[(677, 679)], Noise::new(1, IdleSound::Profile(2055), 5.0, 0.0)),
    (&[(702, 705)], Noise::new(3, IdleSound::Ids { first: 16000, count: 14 }, 2.0, 0.0)),
    (&[(707, 707)], Noise::new(1, IdleSound::Profile(1015), 4.0, 0.0)),
];

fn noises(table: &Table) -> Vec<IdleNoise> {
    let mut out: Vec<IdleNoise> = table
        .iter()
        .flat_map(|(spans, noise)| {
            spans.iter().flat_map(move |&(first, last)| {
                (first..=last).map(move |creature| IdleNoise {
                    creature,
                    roll: noise.roll,
                    sound: noise.sound,
                    wait: noise.wait,
                    rate: noise.rate,
                })
            })
        })
        .collect();
    out.sort_by_key(|noise| noise.creature);
    out
}

/// The idle noises of the types.
pub fn idle_noises() -> IdleNoises {
    IdleNoises {
        hostile: noises(HOSTILE),
        peaceful: noises(PEACEFUL),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_type_makes_one_noise_in_each_list() {
        let noises = idle_noises();
        for list in [&noises.hostile, &noises.peaceful] {
            assert!(list.windows(2).all(|w| w[0].creature < w[1].creature));
            assert!(
                list.iter()
                    .all(|n| (1..=714).contains(&n.creature) && n.roll >= 1)
            );
        }
        assert_eq!((noises.hostile.len(), noises.peaceful.len()), (311, 241));
        let find =
            |list: &[IdleNoise], creature| list.iter().find(|n| n.creature == creature).cloned();
        // The wolves, one in two, of the profile 2014; the thieves one in three, 60 s; the
        // goblins none at peace.
        let wolf = find(&noises.hostile, 512).unwrap();
        assert_eq!(
            (wolf.roll, wolf.sound, wolf.wait),
            (2, IdleSound::Profile(2014), 0.0)
        );
        let thief = find(&noises.hostile, 313).unwrap();
        assert_eq!(
            (thief.roll, thief.sound, thief.wait),
            (3, IdleSound::Profile(2023), 60.0)
        );
        assert!(find(&noises.peaceful, 76).is_none());
        // The pigs, the novices and the townsfolk's murmurs.
        let pig = find(&noises.peaceful, 560).unwrap();
        assert_eq!((pig.roll, pig.sound), (2, IdleSound::Profile(2011)));
        let novice = find(&noises.peaceful, 677).unwrap();
        assert_eq!(
            (novice.roll, novice.sound, novice.wait),
            (1, IdleSound::Profile(2055), 5.0)
        );
        let murmur = find(&noises.peaceful, 280).unwrap();
        assert_eq!(
            murmur.sound,
            IdleSound::Ids {
                first: 16050,
                count: 29
            }
        );
    }
}
