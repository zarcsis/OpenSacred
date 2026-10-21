//! Tests on the GOG installation files: counts from docs/02 §4.1 and a record-by-record comparison
//! with the Python prototype `tools/analysis/formats/grn_dump.py`. CI does not have the game files,
//! so every test here is ignored by default.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;

use super::*;
use crate::testing::{game_file, repo_root, run_python, scratch_dir};

const MODELS_PAK: &str = "PAK/models.pak";
const MODELS03_PAK: &str = "PAK/models03.pak";

fn all_records<'a>(pak: &MdlPak<'a>) -> Vec<Record<'a>> {
    (0..pak.len())
        .map(|i| pak.record(i).unwrap_or_else(|e| panic!("{e}")))
        .collect()
}

/// End of the record's GRN file in the container.
fn grn_end(record: &Record) -> usize {
    usize::try_from(record.data_offset).unwrap() + record.grn().len()
}

#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn model_paks_match_docs() {
    let data = game_file(MODELS_PAK);
    let pak = MdlPak::parse(&data).unwrap();
    assert_eq!((pak.models(), pak.motions()), (1572, 3421));
    assert_eq!(pak.pak().header_fields()[5], 13_268);
    let records = all_records(&pak);

    // Records lie back to back in directory order up to the end of the file.
    let entries = pak.pak().entries();
    for (record, next) in records.iter().zip(&entries[1..]) {
        assert_eq!(grn_end(record), usize::try_from(next.offset).unwrap());
    }
    assert_eq!(grn_end(records.last().unwrap()), data.len());

    let empty: Vec<_> = records
        .iter()
        .filter(|r| r.grn().is_empty())
        .map(|r| (r.index, r.name()))
        .collect();
    assert_eq!(
        empty,
        [(0, b"INVALID_MODEL".as_slice()), (1572, b"INVALID_MOTION")]
    );
    let mut flags = HashMap::new();
    for r in &records {
        *flags.entry((r.kind, r.flags)).or_insert(0) += 1;
    }
    assert_eq!(flags[&(Kind::Model, 3)], 1571);
    assert_eq!(flags[&(Kind::Motion, 5)], 3420);
    assert_eq!(flags.len(), 4);

    let long: Vec<_> = records
        .iter()
        .filter(|r| r.name().len() == NAME_LEN)
        .map(|r| (r.index, r.name()))
        .collect();
    assert_eq!(
        long,
        [(1546, b"VAMP_WEAPON_SWORDLONG_2HANDED.GR".as_slice())]
    );

    let motions: Vec<u32> = records
        .iter()
        .flat_map(|r| r.motions.iter().copied())
        .filter(|&m| m != 0)
        .collect();
    assert_eq!(motions.len(), 5110);
    assert_eq!(motions.iter().max(), Some(&3420));

    // The models' own scale: the same on the three axes, 1.0 but on 36 models, the placeholder
    // among them.
    let mut scales: HashMap<u32, usize> = HashMap::new();
    for r in &records {
        if let Some([x, y, z]) = r.scale {
            assert!(x == y && y == z, "#{}: scale {x}, {y}, {z}", r.index);
            *scales.entry(x.to_bits()).or_insert(0) += 1;
        }
    }
    let mut scales: Vec<(f32, usize)> = scales
        .into_iter()
        .map(|(bits, n)| (f32::from_bits(bits), n))
        .collect();
    scales.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(
        scales,
        [
            (0.33, 4),
            (0.43, 1),
            (0.5, 6),
            (1.0, 1536),
            (1.1, 24),
            (5.0, 1)
        ]
    );
    let gold: Vec<_> = records
        .iter()
        .filter(|r| r.scale == Some([0.33; 3]))
        .map(|r| (r.index, r.name()))
        .collect();
    assert_eq!(
        gold,
        [
            (973, b"GOLDCOINS_01.GRN".as_slice()),
            (974, b"GOLDCOINS_02.GRN"),
            (975, b"GOLDCOINS_05.GRN"),
            (976, b"GOLDCOINS_10.GRN"),
        ]
    );

    let data03 = game_file(MODELS03_PAK);
    let pak03 = MdlPak::parse(&data03).unwrap();
    assert_eq!(pak03.pak().header_fields()[5], 19_897);
    let names: Vec<_> = all_records(&pak03)
        .iter()
        .map(|r| (r.kind, r.name().to_vec(), r.grn().len()))
        .collect();
    assert_eq!(
        names,
        [
            (Kind::Model, b"INVALID_MODEL".to_vec(), 0),
            (Kind::Model, b"KROMBACHER.GRN".to_vec(), 25_768),
            (Kind::Motion, b"INVALID_MOTION".to_vec(), 0),
            (Kind::Motion, b"BO02_ACTIVATE.GRN".to_vec(), 32_756),
        ]
    );
    // `Models.tmp` holds the descriptors of both paks: 1574 models and 3423 motions.
    assert_eq!(pak.models() + pak03.models(), 1574);
    assert_eq!(pak.motions() + pak03.motions(), 3423);
}

/// Every record of both paks against the `--records` table of `grn_dump.py`: kind, name, bounds
/// of the GRN file, flags, and the motion slots and the scale of models.
#[test]
#[ignore = "needs the game files in OriginalGame/ and Python with tools/requirements.txt"]
fn records_match_grn_dump_py() {
    let out = scratch_dir("grn-dump");
    let table = out.join("records.tsv");
    let game_dir = repo_root().join("OriginalGame");
    run_python(
        "tools/analysis/formats/grn_dump.py",
        &[
            OsStr::new("--game-dir"),
            game_dir.as_os_str(),
            OsStr::new("--records"),
            table.as_os_str(),
        ],
    );
    let text = fs::read_to_string(&table).unwrap();
    let mut lines = text.lines();
    let columns: Vec<_> = lines.next().unwrap().split('\t').collect();
    let column = |name: &str| columns.iter().position(|&c| c == name).unwrap();
    let wanted = [
        "pak",
        "index",
        "kind",
        "name",
        "data_offset",
        "data_size",
        "flags",
        "motions",
        "scale",
    ]
    .map(column);

    let mut compared = 0;
    for path in [MODELS_PAK, MODELS03_PAK] {
        let data = game_file(path);
        let pak = MdlPak::parse(&data).unwrap();
        for record in all_records(&pak) {
            let row: Vec<_> = lines.next().unwrap().split('\t').collect();
            let python = wanted.map(|i| row[i]);
            let motions: Vec<_> = record
                .motions
                .iter()
                .enumerate()
                .filter(|&(_, &m)| m != 0)
                .map(|(slot, m)| format!("{slot}:{m}"))
                .collect();
            let ours = [
                path.to_owned(),
                record.index.to_string(),
                match record.kind {
                    Kind::Model => "model",
                    Kind::Motion => "motion",
                }
                .to_owned(),
                String::from_utf8(record.name().to_vec()).unwrap(),
                record.data_offset.to_string(),
                record.grn().len().to_string(),
                record.flags.to_string(),
                motions.join(";"),
                record.scale.map_or_else(String::new, |scale| {
                    scale.map(|s| format!("{:08x}", s.to_bits())).join(",")
                }),
            ];
            assert_eq!(python, ours.each_ref().map(String::as_str));
            compared += 1;
        }
    }
    assert!(lines.next().is_none());
    assert_eq!(compared, 4997);
    fs::remove_dir_all(out).unwrap();
}
