//! Tests on the GOG installation files: counts from docs/02 §3.1 and pixel comparison with the
//! Python prototypes `tools/analysis/formats/tex_decode.py` and `tex_fmt6.py`. CI does not have the
//! game files, so every test here is ignored by default.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::thread;

use super::*;
use crate::testing::{
    PakRecord, build_pak, game_file, read_png, repo_root, run_python, scratch_dir,
};

const TEXTURE_PAK: &str = "PAK/texture.pak";
const TEXTURE03_PAK: &str = "PAK/texture03.pak";

/// Non-empty records of a container in directory order.
fn non_empty_records<'a>(pak: &TexPak<'a>) -> Vec<Record<'a>> {
    (0..pak.len())
        .filter_map(|i| pak.record(i).unwrap())
        .collect()
}

#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn texture_pak_counts_match_docs() {
    let data = game_file(TEXTURE_PAK);
    let pak = TexPak::parse(&data).unwrap();
    assert_eq!(pak.len(), 25_535);
    // `record` checks `dataSize == size` and the directory type for every record.
    let records = non_empty_records(&pak);
    let count = |format| records.iter().filter(|r| r.header.format == format).count();
    assert_eq!(count(Format::Argb4444Zlib), 25_307);
    assert_eq!(count(Format::Bgra8888), 227);
    assert_eq!(pak.len() - records.len(), 1);
    assert!(pak.pak().entries()[0].is_empty());

    assert!(records.iter().all(|r| r.header.packed_offset == 0));

    let data03 = game_file(TEXTURE03_PAK);
    let pak03 = TexPak::parse(&data03).unwrap();
    let records03 = non_empty_records(&pak03);
    let names: Vec<_> = records03.iter().map(|r| r.header.name()).collect();
    assert_eq!(pak03.len(), 3);
    assert_eq!(names, [b"CAB.TGA".as_slice(), b"KROMBACHER.TGA"]);

    // The size distribution in docs/02 comes from `Texture.TMP`, i.e. both paks:
    // `CAB.TGA` (128x128) is in `texture03.pak`.
    let mut sizes = HashMap::new();
    for r in records.iter().chain(&records03) {
        *sizes.entry((r.header.width, r.header.height)).or_insert(0) += 1;
    }
    for (side, expected) in [(256, 22_932), (128, 919), (64, 724), (512, 205), (1024, 21)] {
        assert_eq!(sizes[&(side, side)], expected, "{side}x{side}");
    }
}

#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn every_texture_decodes() {
    for path in [TEXTURE_PAK, TEXTURE03_PAK] {
        let data = game_file(path);
        let pak = TexPak::parse(&data).unwrap();
        let records = non_empty_records(&pak);
        let threads = thread::available_parallelism().map_or(4, |n| n.get());
        let chunk = records.len().div_ceil(threads);
        thread::scope(|s| {
            for part in records.chunks(chunk) {
                s.spawn(move || {
                    for r in part {
                        let texture = r.decode().unwrap_or_else(|e| panic!("{path}: {e}"));
                        assert_eq!(
                            texture.rgba.len(),
                            usize::from(r.header.width) * usize::from(r.header.height) * 4
                        );
                    }
                });
            }
        });
    }
}

/// Output name of `tex_decode.py`: the name decoded as Latin-1 with `.` replaced by `_`.
fn python_name(name: &[u8]) -> String {
    name.iter()
        .map(|&b| if b == b'.' { '_' } else { char::from(b) })
        .collect()
}

/// Format 4 against `tex_decode.py`. The script decodes only the first N records of a pak, so
/// the test assembles a sample pak from records spread over `texture.pak`: every size, `MIX`
/// atlases, names without NUL and the last record, whose `dataSize` runs past the end of the
/// file. Records are copied byte for byte together with their directory type and size.
#[test]
#[ignore = "needs the game files in OriginalGame/ and the Python of pyproject.toml through uv"]
fn argb4444_matches_tex_decode_py() {
    let data = game_file(TEXTURE_PAK);
    let pak = TexPak::parse(&data).unwrap();
    let argb: Vec<_> = non_empty_records(&pak)
        .into_iter()
        .filter(|r| r.header.format == Format::Argb4444Zlib)
        .collect();

    let mut wanted = BTreeSet::new();
    let stride = argb.len() / 48;
    wanted.extend(argb.iter().step_by(stride).map(|r| r.index));
    let mut sizes = HashSet::new();
    for r in &argb {
        if sizes.insert((r.header.width, r.header.height)) || r.header.name().len() == NAME_LEN {
            wanted.insert(r.index);
        }
    }
    let last = argb.last().unwrap().index;
    wanted.insert(last);

    // Python writes one PNG per name, so a name is taken into the sample once.
    let mut taken = HashSet::new();
    let sample: Vec<_> = argb
        .iter()
        .filter(|r| wanted.contains(&r.index) && taken.insert(python_name(r.header.name())))
        .collect();
    assert_eq!(sample.last().unwrap().index, last);

    let entries = pak.pak().entries();
    let records: Vec<_> = sample
        .iter()
        .map(|r| {
            let entry = entries[r.index];
            let start = usize::try_from(entry.offset).unwrap();
            let end = entries[r.index + 1..]
                .iter()
                .find(|e| !e.is_empty())
                .map_or(data.len(), |e| usize::try_from(e.offset).unwrap());
            Some(PakRecord {
                kind: entry.kind,
                size: entry.size,
                bytes: data[start..end].to_vec(),
            })
        })
        .collect();
    let sample_pak = build_pak(MAGIC, VERSION, &records);
    let last_entry = entries[last];
    assert!(
        usize::try_from(last_entry.offset).unwrap()
            + RECORD_HEADER_SIZE
            + usize::try_from(last_entry.size).unwrap()
            > data.len(),
        "the last record must run past the end of texture.pak (docs/02 §3.1)"
    );

    let out = scratch_dir("tex-decode");
    let sample_path = out.join("sample.pak");
    fs::write(&sample_path, &sample_pak).unwrap();
    let max_entries = sample.len().to_string();
    run_python(
        "tools/analysis/formats/tex_decode.py",
        &[
            OsStr::new("--pak"),
            sample_path.as_os_str(),
            OsStr::new("--max-entries"),
            OsStr::new(&max_entries),
            OsStr::new("--out"),
            out.as_os_str(),
        ],
    );

    let sample_pak = TexPak::parse(&sample_pak).unwrap();
    let decoded = non_empty_records(&sample_pak);
    assert_eq!(decoded.len(), sample.len());
    for r in &decoded {
        let name = python_name(r.header.name());
        let texture = r.decode().unwrap_or_else(|e| panic!("{name}: {e}"));
        let png = read_png(&out.join(format!("tex_{name}_4444.png")));
        assert_eq!(png.color, png::ColorType::Rgba, "{name}");
        assert_eq!(
            (png.width, png.height),
            (texture.width, texture.height),
            "{name}"
        );
        assert!(png.pixels == texture.rgba, "{name}: pixels differ");
    }
    fs::remove_dir_all(out).unwrap();
}

/// Format 6 against `tex_fmt6.py`: the script tiles `GUI_OPTION*`, `GUI_CHAR*` and their `UW`
/// variants 4×3 into 1024×768 menu screens in BGRA order and drops alpha, so alpha is checked by
/// the statistics the script prints (and docs/02 §3.1 quotes: 77 % of 255, 11 % of zeros).
#[test]
#[ignore = "needs the game files in OriginalGame/ and the Python of pyproject.toml through uv"]
fn bgra8888_matches_tex_fmt6_py() {
    let data = game_file(TEXTURE_PAK);
    let pak = TexPak::parse(&data).unwrap();
    // The script keys textures by the upper-cased name; a later duplicate replaces an earlier one.
    let mut textures = BTreeMap::new();
    for r in non_empty_records(&pak) {
        if r.header.format == Format::Bgra8888 {
            let name = r.header.name().to_ascii_uppercase();
            textures.insert(name, r.decode().unwrap());
        }
    }

    let out = scratch_dir("tex-fmt6");
    let pak_path = repo_root().join("OriginalGame").join(TEXTURE_PAK);
    let stdout = run_python(
        "tools/analysis/formats/tex_fmt6.py",
        &[
            OsStr::new("--pak"),
            pak_path.as_os_str(),
            OsStr::new("--out"),
            out.as_os_str(),
        ],
    );

    for prefix in ["GUI_OPTION", "GUI_UW_OPTION", "GUI_CHAR", "GUI_UW_CHAR"] {
        let png = read_png(&out.join(format!("{prefix}_bgra.png")));
        assert_eq!(png.color, png::ColorType::Rgb, "{prefix}");
        assert_eq!((png.width, png.height), (1024, 768), "{prefix}");
        // Tile number as the script's `prefix_?(\d+)\.TGA` gives it: `GUI_CHAR01.TGA` is tile 1.
        let tiles: BTreeMap<usize, &Texture> = textures
            .iter()
            .filter_map(|(name, texture)| {
                let rest = name.strip_prefix(prefix.as_bytes())?;
                let rest = rest.strip_prefix(b"_").unwrap_or(rest);
                let digits = rest.strip_suffix(b".TGA")?;
                let number = std::str::from_utf8(digits).ok()?.parse().ok()?;
                digits
                    .iter()
                    .all(u8::is_ascii_digit)
                    .then_some((number, texture))
            })
            .collect();
        assert!(
            tiles.keys().copied().eq(1..=12),
            "{prefix}: {:?}",
            tiles.keys()
        );
        for (&number, texture) in &tiles {
            assert_eq!((texture.width, texture.height), (256, 256));
            let (row, col) = ((number - 1) / 4, (number - 1) % 4);
            for y in 0..256 {
                let mosaic = ((row * 256 + y) * 1024 + col * 256) * 3;
                let (mosaic_row, _) = png.pixels[mosaic..mosaic + 256 * 3].as_chunks::<3>();
                let (tile_row, _) = texture.rgba[y * 256 * 4..(y + 1) * 256 * 4].as_chunks::<4>();
                for (x, (m, &[r, g, b, _])) in mosaic_row.iter().zip(tile_row).enumerate() {
                    assert_eq!(m, &[r, g, b], "{prefix} tile {number}, pixel ({x}, {y})");
                }
            }
        }
    }

    let alpha: Vec<u8> = textures
        .values()
        .flat_map(|t| t.rgba.as_chunks::<4>().0.iter().map(|&[_, _, _, a]| a))
        .collect();
    let share = |value| {
        let n = alpha.iter().filter(|&&a| a == value).count();
        100.0
            * (f64::from(u32::try_from(n).unwrap())
                / f64::from(u32::try_from(alpha.len()).unwrap()))
    };
    let (opaque, transparent) = (share(255), share(0));
    let line = format!("byte 3: ==255 {opaque:5.1}%  ==0 {transparent:5.1}%");
    assert!(
        stdout.lines().any(|l| l.starts_with(&line)),
        "`{line}` not in the output of tex_fmt6.py:\n{stdout}"
    );
    assert_eq!((opaque.round(), transparent.round()), (77.0, 11.0));
    fs::remove_dir_all(out).unwrap();
}
