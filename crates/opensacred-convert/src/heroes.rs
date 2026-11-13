//! `heroes` subcommand: the hero templates `Templates/Hero*.ptx` → one hero file each under
//! `<out>/heroes/` with the `heroes.ron` index (docs/02 §9; the format is
//! [`opensacred_data::heroes`]). Of `save/`, the two heroes that the installation ships for the
//! Underworld campaign's choice, `hero06.pax` and `hero07.pax`, as it ships them, by their
//! SHA-256 (docs/re/underworld.md; docs/decisions/interface.md, "The installation's Underworld
//! heroes"): the player's own saves are not supported (PLAN.md, ADR-08), and a file that an
//! export of the original's overwrote is left alone.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use opensacred_data::heroes::{
    Carried, Hero, HeroArt, HeroEntry, HeroStats, Heroes, INDEX, SavedHero, Section, Socketed, Worn,
};
use opensacred_formats::amh::{Amh, Item, Summary};
use sha2::{Digest, Sha256};

use crate::verify::{GOG_MANIFEST, parse_manifest, read_game_file};

pub fn output_dir(out: &Path) -> PathBuf {
    out.join("heroes")
}

/// The hero templates of the manifest.
fn sources() -> Result<Vec<String>> {
    let mut paths: Vec<String> = parse_manifest(GOG_MANIFEST)?
        .into_iter()
        .map(|e| e.path.to_owned())
        .filter(|p| {
            let lower = p.to_ascii_lowercase();
            lower.starts_with("templates/") && lower.ends_with(".ptx")
        })
        .collect();
    paths.sort();
    Ok(paths)
}

/// Whether the master at `path`, relative to the masters' directory, is one of the heroes' own
/// files (`heroes/templates/*.hero`, `heroes/save/*.hero`), which `build` leaves out of the game's
/// assets (#2650): the game takes what it needs of them from `heroes.ron` alone, which this
/// command makes from them.
pub fn is_unread(path: &str) -> bool {
    path.starts_with("heroes/") && path.ends_with(".hero")
}

/// `Templates/Hero00.ptx` → `templates/hero00.hero`.
fn file_name(source: &str) -> String {
    let lower = source.to_ascii_lowercase();
    let stem = lower.rsplit_once('.').map_or(lower.as_str(), |(s, _)| s);
    format!("{stem}.hero")
}

/// An item's record's words from its `+4`.
fn words(item: &Item) -> Vec<u16> {
    record_words(&item.record)
}

/// A record's words from its `+4`.
fn record_words(record: &[u8]) -> Vec<u16> {
    record
        .get(4..)
        .unwrap_or_default()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&w| u16::from_le_bytes(w))
        .collect()
}

/// The items in an item's sockets, of their records: each one's type, words from `+4` and its
/// smith's tier (`+0x81`).
fn socketed(item: &Item) -> Vec<Socketed> {
    item.socketed
        .iter()
        .map(|record| Socketed {
            item: record
                .first_chunk::<4>()
                .map_or(0, |&bytes| u32::from_le_bytes(bytes)),
            words: record_words(record),
            tier: record.get(0x81).copied().unwrap_or(0),
        })
        .collect()
}

/// The items of a template's inventory that its backpack does not hold, which the hero wears and
/// holds: each record's words from its `+4`.
fn worn(amh: &Amh) -> Result<Vec<Worn>> {
    let packed = amh.backpack()?;
    Ok(amh
        .items()?
        .iter()
        .filter(|item| !packed.iter().any(|packed| packed.id == item.id))
        .map(|item| Worn {
            id: item.id,
            item: item.item,
            words: words(item),
            socketed: socketed(item),
            name: item.name,
        })
        .collect())
}

/// The items of a template's backpack, cell by cell, with their records' words from the inventory.
fn backpack(amh: &Amh) -> Result<Vec<Carried>> {
    let items = amh.items()?;
    amh.backpack()?
        .into_iter()
        .map(|packed| {
            let item = items
                .iter()
                .find(|item| item.id == packed.id)
                .with_context(|| {
                    format!("the backpack's item {} not in the inventory", packed.id)
                })?;
            Ok(Carried {
                id: packed.id,
                item: packed.item,
                at: packed.at,
                size: packed.size,
                words: words(item),
                socketed: socketed(item),
                name: item.name,
            })
        })
        .collect()
}

/// The combat arts of a template's creature section, as it holds them.
fn arts(amh: &Amh) -> Result<Vec<HeroArt>> {
    Ok(amh
        .arts()?
        .into_iter()
        .map(|a| HeroArt {
            table: a.table,
            id: a.id,
            level: a.level,
            bonus: a.bonus,
            flags: a.flags,
            time: a.time,
            factor: a.factor,
            remaining: a.remaining,
        })
        .collect())
}

/// The installation's heroes of the Underworld's choice: their slots, their files in `save/`
/// and their SHA-256 as the GOG installation ships them (22,417 and 23,218 bytes).
const SHIPPED: [(u8, &str, &str); 2] = [
    (
        6,
        "save/hero06.pax",
        "d66a82a06c1d935a5c83e0ff48b0e96cf6b7ddfae17adeb4b5b3cbbd408db392",
    ),
    (
        7,
        "save/hero07.pax",
        "63a064c23a61c93579c946af7840a449e4126c9ef5d2469840e056258aaacd57",
    ),
];

/// Converts the templates, and the installation's Underworld heroes found as it ships them;
/// returns how many.
pub fn convert(game_dir: &Path, out: &Path) -> Result<usize> {
    let dir = output_dir(out);
    let mut heroes = Vec::new();
    for source in sources()? {
        let data = read_game_file(&game_dir.join(&source))?;
        heroes.push(convert_one(&dir, source, &data)?.0);
    }
    let mut saved = Vec::new();
    for (slot, source, sha) in SHIPPED {
        let Ok(data) = fs::read(game_dir.join(source)) else {
            continue;
        };
        let found = Sha256::digest(&data)
            .iter()
            .fold(String::new(), |hex, byte| hex + &format!("{byte:02x}"));
        if found != sha {
            continue;
        }
        let (hero, summary) = convert_one(&dir, source.to_owned(), &data)?;
        saved.push(SavedHero {
            slot,
            name: summary.name,
            hero,
            equipment: (0u8..)
                .zip(summary.equipment)
                .filter(|&(_, item)| item != 0)
                .collect(),
            won: summary.won.to_vec(),
        });
    }
    let count = heroes.len() + saved.len();
    let text = ron::ser::to_string_pretty(&Heroes { heroes, saved }, crate::utf8::pretty())
        .context("writing heroes.ron")?;
    let path = dir.join(INDEX);
    fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
    Ok(count)
}

/// Converts the hero file `source` of the bytes `data` into its hero file under `dir`; returns
/// its index's entry and its summary.
fn convert_one(dir: &Path, source: String, data: &[u8]) -> Result<(HeroEntry, Summary)> {
    {
        let amh = Amh::parse(data).with_context(|| source.clone())?;
        let info = amh.summary().with_context(|| source.clone())?;
        let worn = worn(&amh).with_context(|| source.clone())?;
        let backpack = backpack(&amh).with_context(|| source.clone())?;
        let arts = arts(&amh).with_context(|| source.clone())?;
        let stats = amh.stats().with_context(|| source.clone())?;
        let equipment = (0u8..)
            .zip(amh.equipment().with_context(|| source.clone())?)
            .filter(|&(_, id)| id != 0)
            .collect();
        let sets = amh.sets().with_context(|| source.clone())?.to_vec();
        let (slots, selected) = amh.art_slots().with_context(|| source.clone())?;
        let art_slots = slots.iter().map(|banks| banks[0]).collect();
        let stats = HeroStats {
            experience: stats.experience,
            gold: stats.gold,
            spent: stats.spent,
            attribute_points: stats.attribute_points,
            skills: stats.skills,
            bases: stats.bases,
            skill_points: stats.skill_points,
            to_learn: stats.to_learn,
        };
        let hero = Hero {
            sections: amh
                .sections
                .into_iter()
                .map(|s| {
                    Ok(Section {
                        slot: u8::try_from(s.slot)?,
                        tag: s.tag,
                        size: s.size,
                        data: s.data,
                    })
                })
                .collect::<Result<_>>()?,
        };
        let file = file_name(&source);
        let path = dir.join(&file);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let bytes = hero
            .encode()
            .with_context(|| format!("encoding {source}"))?;
        fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))?;
        Ok((
            HeroEntry {
                source,
                file,
                level: info.level,
                class: info.class,
                worn,
                backpack,
                arts,
                stats,
                equipment,
                sets,
                art_slots,
                selected,
            },
            info,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::scratch_dir;

    #[test]
    fn the_heroes_files_are_unread_and_their_index_read() {
        assert!(is_unread("heroes/templates/hero00.hero"));
        assert!(is_unread("heroes/save/hero06.hero"));
        assert!(!is_unread("heroes/heroes.ron"));
        assert!(!is_unread("netscript/hero00.hero"));
    }

    #[test]
    fn hero_files_keep_their_directory() {
        assert_eq!(file_name("Templates/Hero00.ptx"), "templates/hero00.hero");
        assert_eq!(sources().unwrap().len(), 8);
    }

    /// Every hero converts and reads back; each template is of another class.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn every_hero_converts() {
        let out = scratch_dir("heroes");
        let game = opensacred_locate::original();
        assert_eq!(convert(&game, &out).unwrap(), 10);
        let dir = output_dir(&out);
        // The ten heroes' files, which `build` leaves out (#2650), and the index, which it keeps.
        let mut files = Vec::new();
        crate::manifest::files(&out, &dir, &mut files).unwrap();
        let unread = files.iter().filter(|f| is_unread(f)).count();
        assert_eq!((files.len(), unread), (11, 10), "{files:?}");
        let index: Heroes = ron::from_str(&fs::read_to_string(dir.join(INDEX)).unwrap()).unwrap();
        // The installation's Underworld heroes: the Dwarf in the slot 6 and the Daemon in the
        // slot 7, of level 29, each with its name and its whole file.
        let saved: Vec<_> = index
            .saved
            .iter()
            .map(|s| (s.slot, s.hero.class, s.hero.level, s.name.is_empty()))
            .collect();
        assert_eq!(saved, [(6, 8, 29, false), (7, 9, 29, false)]);
        // What their figures wear, by their summaries' slots: the Dwarf's 15 items, his musket
        // in the right hand, and the Daemon's 14, her sword in it and her shield in the left; no
        // difficulty won, so bronze and silver are offered.
        let worn: Vec<_> = index
            .saved
            .iter()
            .map(|s| {
                let hand = |slot| s.equipment.iter().find(|e| e.0 == slot).map(|e| e.1);
                (s.equipment.len(), hand(13), hand(12), s.won.clone())
            })
            .collect();
        assert_eq!(
            worn,
            [
                (15, Some(1856), Some(3096), vec![false; 6]),
                (14, Some(1243), Some(1246), vec![false; 6])
            ]
        );
        // Their values, as the original's driver prints them of the heroes started (docs/02 §9):
        // the Dwarf's 14 points on each of two attributes, his five skills and nine points.
        let dwarf = index.saved[0].hero.stats;
        assert_eq!(
            (
                dwarf.experience,
                dwarf.gold,
                dwarf.spent,
                dwarf.skill_points,
                dwarf.to_learn
            ),
            (1_800_000, 977_384, [0, 0, 14, 14, 0, 0], 9, 1)
        );
        assert_eq!(dwarf.skills, [2, 10, 11, 5, 29, 0, 0, 0]);
        // His equipment by the items' ids, his fourth set in his hands, and his two socketed
        // items' records.
        let saved = &index.saved[0].hero;
        assert_eq!(
            (
                saved.equipment.len(),
                saved.equipment.iter().find(|e| e.0 == 13)
            ),
            (15, Some(&(13, 74)))
        );
        assert_eq!(saved.sets, [[69, 0], [70, 71], [0, 72], [73, 74], [0, 0]]);
        // The names that the item maker composed for his backpack's made items, as the original's
        // objects keep them (`+0x3C`, read by a trace, #2390): the ring 4922's `GEN_GR_FE_RING2`
        // and the pistol 1855's `GEN_GR_PH_PISTOLE2`, the bit 31 set; a plain item's 0.
        let name = |id: u32| saved.backpack.iter().find(|c| c.id == id).map(|c| c.name);
        assert_eq!(
            [name(27), name(32), name(23)],
            [Some(0x866A_F7CF), Some(0xA08B_3791), Some(0)]
        );
        assert_eq!(
            opensacred_data::text::sacred_hash(b"GEN_GR_FE_RING2"),
            0x866A_F7CF & 0x7FFF_FFFF
        );
        // His bar's four weapon arts, his fourth set and his second slot selected.
        assert_eq!(
            (saved.art_slots.clone(), saved.selected),
            (
                vec![(3, 1080), (3, 1082), (3, 1085), (3, 1087), (0, 0)],
                [3, 1]
            )
        );
        let socketed: Vec<_> = saved
            .worn
            .iter()
            .flat_map(|worn| {
                worn.socketed
                    .iter()
                    .map(|s| (worn.item, s.item, s.words.len()))
            })
            .collect();
        assert_eq!(socketed, [(7725, 5560, 63), (3008, 5557, 63)]);
        assert_eq!(dwarf.bases, [23, 13, 10, 10, 13, 0, 0, 0]);
        // A template starts with its two skills at the base 1 and 5,000 gold.
        let template = index.heroes[0].stats;
        assert_eq!(
            (
                template.experience,
                template.gold,
                template.bases[..2].to_vec()
            ),
            (0, 5_000, vec![1, 1])
        );
        for saved in &index.saved {
            let hero = Hero::decode(&fs::read(dir.join(&saved.hero.file)).unwrap()).unwrap();
            assert_eq!(hero.sections.len(), 8);
        }
        let mut classes: Vec<u32> = index.heroes.iter().map(|h| h.class).collect();
        classes.sort_unstable();
        classes.dedup();
        assert_eq!(classes.len(), 8);
        for entry in &index.heroes {
            let hero = Hero::decode(&fs::read(dir.join(&entry.file)).unwrap()).unwrap();
            assert_eq!(hero.sections.len(), 8);
            assert!((1..=3).contains(&entry.worn.len()), "{}", entry.source);
            assert!(entry.worn.iter().all(|w| w.words.len() == 63));
            // Three healing potions down the first column and the large torch below them.
            let backpack: Vec<_> = entry
                .backpack
                .iter()
                .map(|c| (c.at, c.size, c.item))
                .collect();
            assert_eq!(
                backpack,
                [
                    ((0, 0), (1, 1), 5171),
                    ((0, 1), (1, 1), 5171),
                    ((0, 2), (1, 1), 5171),
                    ((0, 3), (1, 2), 5633)
                ],
                "{}",
                entry.source
            );
            assert!(entry.backpack.iter().all(|c| c.words.len() == 63));
        }
        // The Seraphim's weapon, the record 7901 in her right hand (docs/02 §9): physical damage
        // 21 to 23, `A` 5, `D` 2.
        let seraphim = index.heroes.iter().find(|h| h.class == 1).unwrap();
        // Her combat arts: 1071, and 18 taught by a rune (docs/re/arts.md).
        let arts: Vec<_> = seraphim
            .arts
            .iter()
            .map(|a| (a.table, a.id, a.level, a.flags, a.time))
            .collect();
        assert_eq!(arts, [(2, 1071, 1, 0, 15.0), (1, 18, 1, 1, 24.0)]);
        let counts: Vec<_> = index.heroes.iter().map(|h| h.arts.len()).collect();
        assert_eq!(counts.iter().sum::<usize>(), 19);
        let blade = seraphim.worn.iter().find(|w| w.item == 7901).unwrap();
        assert_eq!(blade.id, 18);
        assert_eq!(
            [
                blade.words[12],
                blade.words[16],
                blade.words[20],
                blade.words[21]
            ],
            [21, 23, 5, 2]
        );
        fs::remove_dir_all(out).unwrap();
    }
}
