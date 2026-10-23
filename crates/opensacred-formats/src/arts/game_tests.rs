//! Tests on the GOG installation files: every record of the two tables against the rows of the
//! Python prototype `tools/analysis/formats/arts_dump.py`, and the records docs/re/arts.md reads.
//! CI does not have the game files, so the tests are ignored by default.

use std::ffi::OsStr;
use std::fs;

use super::*;
use crate::testing::{game_file, repo_root, run_python, scratch_dir};

fn joined<T: ToString>(values: impl IntoIterator<Item = T>) -> String {
    values
        .into_iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn bits(values: [f32; 2]) -> String {
    joined(values.map(|v| format!("{:08x}", v.to_bits())))
}

fn head_row(head: &Head) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}",
        head.id,
        head.name,
        head.description,
        head.icon,
        joined(head.sprites)
    )
}

#[test]
#[ignore = "needs the game files in OriginalGame/ and Python with tools/requirements.txt"]
fn every_record_matches_arts_dump_py() {
    let out = scratch_dir("arts");
    let rows = out.join("rows.tsv");
    let game = repo_root().join("OriginalGame");
    run_python(
        "tools/analysis/formats/arts_dump.py",
        &[
            OsStr::new("--game-dir"),
            game.as_os_str(),
            OsStr::new("--rows"),
            rows.as_os_str(),
        ],
    );
    let exe = game_file("Sacred.exe");
    let mut ours = Vec::new();
    for (i, s) in spells(&exe).unwrap().iter().enumerate() {
        ours.push(format!(
            "spell\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            i + 1,
            head_row(&s.head),
            s.launch,
            joined(s.regeneration),
            joined(s.effects.iter().flatten()),
            joined(s.values.iter().flatten()),
            joined(s.caps),
            s.element,
            s.classes,
            s.flags
        ));
    }
    for (i, a) in weapon_arts(&exe).unwrap().iter().enumerate() {
        ours.push(format!(
            "weapon-art\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            i + 1,
            head_row(&a.head),
            a.value_3c,
            bits(a.regeneration),
            a.distance,
            a.launch,
            bits(a.factor),
            bits(a.value_58),
            joined(a.value_60),
            joined(a.value_68),
            a.classes,
            a.flags
        ));
    }
    let theirs = fs::read_to_string(&rows).unwrap();
    let theirs: Vec<&str> = theirs.lines().skip(1).collect();
    assert_eq!(ours.len(), theirs.len());
    for (a, b) in ours.iter().zip(&theirs) {
        assert_eq!(a, b);
    }
    fs::remove_dir_all(out).unwrap();
}

/// The counts, the ids and the records of #505's "How it is checked" (docs/re/arts.md).
#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn the_tables_hold_the_arts_docs_re_reads() {
    let exe = game_file("Sacred.exe");
    let spells = spells(&exe).unwrap();
    assert_eq!(spells.len(), 70);
    let mut ids: Vec<u16> = spells.iter().map(|s| s.head.id).collect();
    // 13 and 14, and 47 and 48, are swapped; from 54 on the ids run ahead of the records.
    assert_eq!((ids[12], ids[13], ids[46], ids[47]), (14, 13, 48, 47));
    ids.sort_unstable();
    let expected: Vec<u16> = (1..=72).filter(|id| ![54, 68].contains(id)).collect();
    assert_eq!(ids, expected);

    let spell = |id| spells.iter().find(|s| s.head.id == id).unwrap();
    // `GUI_SPELL_IRRITATION`: 15 + 9·L seconds, the value 0 14 + L under its cap 30, the
    // Seraphim alone.
    let irritation = spell(18);
    assert_eq!(
        (irritation.head.name, irritation.head.description),
        (816, 9247)
    );
    assert_eq!(irritation.head.icon, "GUI_SPELL_IRRITATION.TGA");
    assert_eq!(irritation.head.sprites, [558, 559, 560]);
    assert_eq!(irritation.launch, 1);
    assert_eq!(irritation.regeneration, [15, 9]);
    assert_eq!((irritation.values[0], irritation.caps[0]), ([14, 1], 30));
    assert_eq!((irritation.element, irritation.classes), (0, 1));
    // `GUI_SPELL_STEINHAUT`: 40 + 24·L seconds; 15 + 5·L % of defence, 25 + 40·L of the physical
    // resistance and 10 + 15·L of the poison's; earth; the Mage.
    let stone_skin = spell(2);
    assert_eq!(stone_skin.regeneration, [40, 24]);
    assert_eq!(stone_skin.effects[5], [15, 5]);
    assert_eq!(stone_skin.effects[6], [25, 40]);
    assert_eq!(stone_skin.effects[9], [10, 15]);
    let others = [0, 1, 2, 3, 4, 7, 8, 10, 11, 12];
    assert!(others.iter().all(|&k| stone_skin.effects[k] == [0, 0]));
    assert_eq!((stone_skin.element, stone_skin.classes), (3, 4));

    let arts = weapon_arts(&exe).unwrap();
    assert_eq!(arts.len(), 94);
    let mut ids: Vec<u16> = arts.iter().map(|a| a.head.id).collect();
    ids.sort_unstable();
    let missing = [1011, 1055, 1056, 1077, 1081, 1088, 1090, 1093, 1095, 1096];
    let expected: Vec<u16> = (1000..=1103).filter(|id| !missing.contains(id)).collect();
    assert_eq!(ids, expected);
    let art = |id| arts.iter().find(|a| a.head.id == id).unwrap();
    // The Seraphim's `Attacke`: 10 + 5·L seconds, made from 130, the factor 0.75 + 0.05·L.
    let attack = art(1071);
    assert_eq!((attack.head.name, attack.head.description), (8266, 9166));
    assert_eq!(attack.head.icon, "GUI_MOVE_ATTACKE.TGA");
    assert_eq!(attack.regeneration, [10.0, 5.0]);
    assert_eq!((attack.distance, attack.launch), (130, 1));
    assert_eq!(attack.factor, [0.75, 0.05]);
    assert_eq!((attack.classes, attack.flags), (1, 1));
    // The Gladiator's `Rundum`: 5 + 3·L seconds, the factor 0.8 + 0.1·L, the radius 160 + 20·L %.
    let round = art(1057);
    assert_eq!(round.regeneration, [5.0, 3.0]);
    assert_eq!(round.factor, [0.8, 0.1]);
    assert_eq!(round.value_60, [160, 20]);
    assert_eq!((round.distance, round.classes), (130, 2));
    // The Gladiator's `Attacke`, the other starting art.
    assert_eq!(
        (art(1059).factor, art(1059).regeneration, art(1059).classes),
        ([0.75, 0.05], [10.0, 5.0], 2)
    );
}
