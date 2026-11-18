//! The scale that the client gives 24 models as it loads its items table (`0x429950`, through the
//! model manager's `0x4134F0` for each type's model `+0x70`), in place of their records' own
//! (`ModelDesc +0x470`, every one of them 1 on file; docs/02 §4.1, #2578): the hornets, the
//! larvae, the giant fly, the slime worms, the Fetzers, the brain-footers, the killer crabs, a
//! pus flower, a hell golem, the wood god and his child, and two bubbles. The client's
//! `GrannyTransformModel` and `GrannyTransformAnimation` then scale their meshes, skeletons and
//! slots' motions by it (`0x411F50`, `0x4118B0`), as `models` does. Facts of the original's
//! load, each a model by its name and the scale the client sets; not a repair of a defect. The
//! converter carries them as its constants (docs/08 §3), which a test checks against the load's
//! code in the user's own `Sacred.exe` and the types' names in `Items.pak`.

/// The models the client scales as it loads (`0x429950`), by their names in `models.pak`, and
/// their scales: the floats it passes, `0x3E6B851F` 0.23, `0x3EA8F5C3` 0.33, `0x3EDC28F6` 0.43,
/// `0x3F07AE14` 0.53, `0x3F2147AE` 0.63, `0x3F3AE148` 0.73, `0x3F4CCCCD` 0.8, `0x3F547AE1` 0.83
/// and `0x3F8CCCCD` 1.1; each with the type whose model it is, by which the load names it.
const SCALES: [(u16, &[u8], f32); 24] = [
    (6091, b"BUBBLE_01.GRN", 0.23),
    (6092, b"BUBBLE_02.GRN", 0.23),
    (191, b"INSEKTENLARVE.GRN", 0.23), // the insect larva
    (353, b"INSEKTENLARVE01.GRN", 0.33),
    (354, b"INSEKTENLARVE02.GRN", 0.43), // the larvae's queen
    (248, b"SCHLEIMWURM02.GRN", 0.23),   // the small slime worm
    (195, b"SCHLEIMWURM.GRN", 0.43),
    (247, b"SCHLEIMWURM01.GRN", 0.83),  // the slime worms' queen
    (193, b"RIESENFLIEGE.GRN", 0.43),   // the giant fly
    (190, b"HORNISSE.GRN", 0.33),       // the hornet
    (335, b"HORNISSE01.GRN", 0.43),     // the hornet hunter
    (336, b"HORNISSE02.GRN", 0.53),     // the hornet queen
    (203, b"GEHIRNFUESSLER.GRN", 0.83), // the brain-footer
    (351, b"GEHIRNFUESSLER01.GRN", 0.43),
    (352, b"GEHIRNFUESSLER02.GRN", 0.43),
    (209, b"KILLERKRABBE.GRN", 0.33), // the killer crab
    (347, b"KILLERKRABBE01.GRN", 0.63),
    (348, b"KILLERKRABBE02.GRN", 1.1),
    (202, b"FETZER.GRN", 0.53),
    (344, b"FETZER01.GRN", 0.73),
    (333, b"EITERBLUME01.GRN", 0.53),
    (350, b"HOELLENGOLEM01.GRN", 0.73),
    (144, b"WOODGOD.GRN", 0.8),
    (145, b"WOODGOD_CHILD.GRN", 0.23),
];

/// The scale the client gives the model named `name` as it loads, if it is one of [`SCALES`].
pub fn scale(name: &[u8]) -> Option<f32> {
    SCALES
        .iter()
        .find(|(_, model, _)| model.eq_ignore_ascii_case(name))
        .map(|&(_, _, scale)| scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scales_are_the_clients_floats() {
        let bits: Vec<u32> = SCALES.iter().map(|(_, _, scale)| scale.to_bits()).collect();
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
        let mut names: Vec<&[u8]> = SCALES.iter().map(|(_, name, _)| *name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SCALES.len());
    }

    /// The scales are those that the load passes in the user's `Sacred.exe`, each for its type's
    /// model, and the type's record of `Items.pak` names that model. The load (`0x429950`, 1,065
    /// bytes) takes a type's model from its table (`MOV reg, [ESI + 0x80 · type + 0x70]`),
    /// pushes the scale three times, pushes the model, asks for the model manager (`0x4115D0`)
    /// and calls its `0x4134F0`: 29 bytes before each of the 24 calls.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn the_scales_are_those_the_executable_passes_for_the_types_models() {
        const LOAD: u32 = 0x0042_9950;
        const LENGTH: usize = 0x429;
        const SET_SCALE: u32 = 0x0041_34F0;
        let game = opensacred_locate::original();
        let exe = std::fs::read(game.join("Sacred.exe")).unwrap();
        let pe = opensacred_formats::pe::Image::parse(&exe).unwrap();
        let code = pe.at(LOAD, LENGTH, "the items' load").unwrap().rest();
        let word = |at: usize| u32::from_le_bytes(code[at..at + 4].try_into().unwrap());
        let mut passed: Vec<(u16, u32)> = Vec::new();
        for call in 29..code.len() - 4 {
            // A call of the scale's setter: `E8` and its target from the instruction after it.
            let after = LOAD + u32::try_from(call).unwrap() + 5;
            if code[call] != 0xE8 || after.wrapping_add(word(call + 1)) != SET_SCALE {
                continue;
            }
            // The 21 bytes before the model's push: the model's load of 6 bytes among the
            // three pushes of 5, after none to all three of them.
            let block = call - 29;
            let (model, scales) = (0..=3)
                .find_map(|before| {
                    let load = block + 5 * before;
                    let pushes: Vec<usize> = (0..3)
                        .map(|n| block + 5 * n + if n < before { 0 } else { 6 })
                        .collect();
                    let loads = code[load] == 0x8B && [0x86, 0x8E, 0x96].contains(&code[load + 1]);
                    (loads && pushes.iter().all(|&at| code[at] == 0x68)).then(|| {
                        let scales: Vec<u32> = pushes.iter().map(|&at| word(at + 1)).collect();
                        (word(load + 2), scales)
                    })
                })
                .expect("a model's load and its scale's pushes before the call");
            assert!(
                scales.iter().all(|&scale| scale == scales[0]),
                "{scales:x?}"
            );
            assert_eq!((model - 0x70) % 0x80, 0, "{model:#x}");
            passed.push((u16::try_from((model - 0x70) / 0x80).unwrap(), scales[0]));
        }
        let mut table: Vec<(u16, u32)> = SCALES
            .iter()
            .map(|&(kind, _, scale)| (kind, scale.to_bits()))
            .collect();
        passed.sort_unstable();
        table.sort_unstable();
        assert_eq!(passed, table);

        let items = std::fs::read(game.join("PAK/Items.pak")).unwrap();
        let items = opensacred_formats::items::Items::parse(&items).unwrap();
        for (kind, name, _) in SCALES {
            let item = items.get(usize::from(kind)).unwrap().unwrap();
            assert!(
                item.name.eq_ignore_ascii_case(name),
                "{kind}: {}",
                item.name.escape_ascii()
            );
        }
    }
}
