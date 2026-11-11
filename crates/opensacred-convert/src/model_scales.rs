//! The scale that the client gives 24 models as it loads its items table (`0x429950`, through the
//! model manager's `0x4134F0` for each type's model `+0x70`), in place of their records' own
//! (`ModelDesc +0x470`, every one of them 1 on file; docs/02 §4.1, #2578): the hornets, the
//! larvae, the giant fly, the slime worms, the Fetzers, the brain-footers, the killer crabs, a
//! pus flower, a hell golem, the wood god and his child, and two bubbles. The client's
//! `GrannyTransformModel` and `GrannyTransformAnimation` then scale their meshes, skeletons and
//! slots' motions by it (`0x411F50`, `0x4118B0`), as `models` does. Facts of the original's
//! load, each a model by its name and the scale the client sets; not a repair of a defect.

/// The models the client scales as it loads (`0x429950`), by their names in `models.pak`, and
/// their scales: the floats it passes, `0x3E6B851F` 0.23, `0x3EA8F5C3` 0.33, `0x3EDC28F6` 0.43,
/// `0x3F07AE14` 0.53, `0x3F2147AE` 0.63, `0x3F3AE148` 0.73, `0x3F4CCCCD` 0.8, `0x3F547AE1` 0.83
/// and `0x3F8CCCCD` 1.1. The types that take them are in the comments.
const SCALES: [(&[u8], f32); 24] = [
    (b"BUBBLE_01.GRN", 0.23),        // 6091
    (b"BUBBLE_02.GRN", 0.23),        // 6092
    (b"INSEKTENLARVE.GRN", 0.23),    // 191, the insect larva
    (b"INSEKTENLARVE01.GRN", 0.33),  // 353
    (b"INSEKTENLARVE02.GRN", 0.43),  // 354, the larvae's queen
    (b"SCHLEIMWURM02.GRN", 0.23),    // 248, the small slime worm
    (b"SCHLEIMWURM.GRN", 0.43),      // 195
    (b"SCHLEIMWURM01.GRN", 0.83),    // 247, the slime worms' queen
    (b"RIESENFLIEGE.GRN", 0.43),     // 193, the giant fly
    (b"HORNISSE.GRN", 0.33),         // 190, the hornet
    (b"HORNISSE01.GRN", 0.43),       // 335, the hornet hunter
    (b"HORNISSE02.GRN", 0.53),       // 336, the hornet queen
    (b"GEHIRNFUESSLER.GRN", 0.83),   // 203, the brain-footer
    (b"GEHIRNFUESSLER01.GRN", 0.43), // 351
    (b"GEHIRNFUESSLER02.GRN", 0.43), // 352
    (b"KILLERKRABBE.GRN", 0.33),     // 209, the killer crab
    (b"KILLERKRABBE01.GRN", 0.63),   // 347
    (b"KILLERKRABBE02.GRN", 1.1),    // 348
    (b"FETZER.GRN", 0.53),           // 202
    (b"FETZER01.GRN", 0.73),         // 344
    (b"EITERBLUME01.GRN", 0.53),     // 333
    (b"HOELLENGOLEM01.GRN", 0.73),   // 350
    (b"WOODGOD.GRN", 0.8),           // 144
    (b"WOODGOD_CHILD.GRN", 0.23),    // 145
];

/// The scale the client gives the model named `name` as it loads, if it is one of [`SCALES`].
pub fn scale(name: &[u8]) -> Option<f32> {
    SCALES
        .iter()
        .find(|(model, _)| model.eq_ignore_ascii_case(name))
        .map(|&(_, scale)| scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scales_are_the_clients_floats() {
        let bits: Vec<u32> = SCALES.iter().map(|(_, scale)| scale.to_bits()).collect();
        for value in bits {
            assert!(
                [
                    0x3E6B_851F,
                    0x3EA8_F5C3,
                    0x3EDC_28F6,
                    0x3F07_AE14,
                    0x3F21_47AE,
                    0x3F3A_E148,
                    0x3F4C_CCCD,
                    0x3F54_7AE1,
                    0x3F8C_CCCD
                ]
                .contains(&value),
                "{value:#x}"
            );
        }
    }

    #[test]
    fn a_listed_model_takes_its_scale_whatever_its_case_and_another_none() {
        assert_eq!(scale(b"HORNISSE.GRN"), Some(0.33));
        assert_eq!(scale(b"Hornisse02.grn"), Some(0.53));
        assert_eq!(scale(b"KILLERKRABBE02.GRN"), Some(1.1));
        assert_eq!(scale(b"WOLF.GRN"), None);
        assert_eq!(scale(b"HORNISSE"), None);
        // Each model once.
        let mut names: Vec<&[u8]> = SCALES.iter().map(|(name, _)| *name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SCALES.len());
    }
}
