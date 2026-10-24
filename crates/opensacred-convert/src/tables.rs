//! `tables` subcommand: `Creature.pak`, `Weapon.pak`, `Items.pak` with the overlay
//! `Items03.pak`, `MOTIONS.PAK`, `Triggers.PAK`, the data tables of `bin/`, and the combat arts'
//! tables and the creatures' voices of `Sacred.exe` → one RON file each under `<out>/tables/`
//! (docs/02 §6; the format is [`opensacred_data::tables`]).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use opensacred_data::tables::{
    ARMOUR_SWITCHES, ARTS, Arts, BALANCE, Balance, BalanceList, BalanceRow, BalanceValue,
    BalanceVariable, CREATURES, Creature, EffectChance, IDLE_NOISES, ITEM_SETS, ITEMS, Item,
    ItemSet, MERCENARIES, MERCENARIES_ADDON, MOTIONS, Mercenary, Motion, QualityPass,
    QualityWeight, STAIRS, START_POINTS, STATIC_REMAP, Spell, StartPoint, TRIGGERS, Trigger,
    VOICES, WEAPON_GROUPS, WEAPON_MODIFIERS, WEAPONS, WORLD_INDEX, WORLD_MAP, Weapon, WeaponArt,
    WeaponModifier,
};
use opensacred_formats::arts::{self, Head};
use opensacred_formats::bin_tables;
use opensacred_formats::items::{self, Items};
use opensacred_formats::mdl::{Kind, MdlPak};
use opensacred_formats::tables;
use opensacred_formats::text::sacred_hash;
use serde::Serialize;

use crate::verify::{read_game_file, read_reference_file};

/// How many indices a short description's text lies after the long one's (`0x5893B0`).
const SHORT_DESCRIPTION: u32 = 600;

pub fn output_dir(out: &Path) -> PathBuf {
    out.join("tables")
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub creatures: usize,
    pub weapons: usize,
    pub items: usize,
    pub motions: usize,
    pub triggers: usize,
    /// The files of `bin/`.
    pub bin_tables: usize,
    pub spells: usize,
    pub weapon_arts: usize,
    /// The creature types that speak.
    pub voices: usize,
    /// The creature types that make an idle noise, hostile and not.
    pub idle_noises: (usize, usize),
}

fn read(game_dir: &Path, path: &str) -> Result<Vec<u8>> {
    read_game_file(&game_dir.join(path))
}

fn write<T: Serialize>(dir: &Path, file: &str, value: &T) -> Result<()> {
    // One record per line block, its arrays on one line each.
    let config = crate::utf8::pretty().compact_arrays(true);
    let text =
        ron::ser::to_string_pretty(value, config).with_context(|| format!("writing {file}"))?;
    let path = dir.join(file);
    fs::write(&path, text).with_context(|| format!("writing {}", path.display()))
}

fn item(slot: usize, r: items::Record) -> Result<Item> {
    Ok(Item {
        slot: u32::try_from(slot)?,
        kind: r.kind,
        class: r.class,
        subclass: r.subclass,
        name: r.name,
        texture: r.texture,
        sprite: r.sprite,
        byte_03: r.byte_03,
        words_08: r.words_08,
        words_14: r.words_14,
        bytes_34: r.bytes_34,
        byte_57: r.byte_57,
        words_58: r.words_58,
    })
}

/// The used slots of `Items.pak`, with those of the overlay `Items03.pak` in slots the base
/// leaves empty.
fn items(game_dir: &Path) -> Result<Vec<Item>> {
    let base_data = read(game_dir, "PAK/Items.pak")?;
    let base = Items::parse(&base_data).context("PAK/Items.pak")?;
    let overlay_data = read(game_dir, "PAK/Items03.pak")?;
    let overlay = Items::parse(&overlay_data).context("PAK/Items03.pak")?;
    let mut out = Vec::new();
    for slot in 0..base.len().max(overlay.len()) {
        let record = if slot < base.len() {
            base.record(slot).context("PAK/Items.pak")?
        } else {
            None
        };
        let extra = if slot < overlay.len() {
            overlay.record(slot).context("PAK/Items03.pak")?
        } else {
            None
        };
        anyhow::ensure!(
            record.is_none() || extra.is_none(),
            "PAK/Items03.pak: slot {slot} is used in PAK/Items.pak too"
        );
        if let Some(r) = record.or(extra) {
            out.push(item(slot, r)?);
        }
    }
    Ok(out)
}

/// The names of the motions in the global order: `models.pak`, then `models03.pak`.
fn motion_names(game_dir: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for path in ["PAK/models.pak", "PAK/models03.pak"] {
        let data = read(game_dir, path)?;
        let pak = MdlPak::parse(&data).context(path)?;
        for index in 0..pak.len() {
            if pak.kind(index) == Kind::Motion {
                let record = pak.record(index).context(path)?;
                names.push(record.name().iter().map(|&b| char::from(b)).collect());
            }
        }
    }
    Ok(names)
}

pub fn convert(game_dir: &Path, out: &Path) -> Result<Summary> {
    let dir = output_dir(out);
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    let creatures: Vec<Creature> = tables::creatures(&read(game_dir, "PAK/Creature.pak")?)
        .context("PAK/Creature.pak")?
        .into_iter()
        .map(|c| Creature {
            id: c.id,
            class: c.class,
            flags: c.flags,
            experience: c.experience,
            base: c.base,
            skills: c.skills,
            skills_extra: c.skills_extra,
            speed: c.speed,
            bonus: c.bonus,
            bonus_value: c.bonus_value,
            bonus_class: c.bonus_class,
            damping: c.damping,
        })
        .collect();
    write(&dir, CREATURES, &creatures)?;

    let weapons: Vec<Weapon> = tables::weapons(&read(game_dir, "PAK/Weapon.pak")?)
        .context("PAK/Weapon.pak")?
        .into_iter()
        .map(|w| Weapon {
            id: w.id,
            parent: w.parent,
            name: w.name,
            floats_00: w.floats_00,
            bytes_14: w.bytes_14,
            float_18: w.float_18,
            bytes_1c: w.bytes_1c,
            words_84: w.words_84.to_vec(),
        })
        .collect();
    write(&dir, WEAPONS, &weapons)?;

    let items = items(game_dir)?;
    write(&dir, ITEMS, &items)?;

    let names = motion_names(game_dir)?;
    let params = tables::motions(&read(game_dir, "PAK/MOTIONS.PAK")?).context("PAK/MOTIONS.PAK")?;
    anyhow::ensure!(
        names.len() == params.len(),
        "PAK/MOTIONS.PAK holds {} motions, the model paks {}",
        params.len(),
        names.len()
    );
    let motions: Vec<Motion> = names
        .into_iter()
        .zip(params)
        .map(|(name, m)| Motion {
            name,
            value_06: m.value_06,
            floats_46: m.floats_46,
            floats_62: m.floats_62,
            value_be: m.value_be,
        })
        .collect();
    write(&dir, MOTIONS, &motions)?;

    let triggers = tables::triggers(&read(game_dir, "World/Triggers.PAK")?)
        .context("World/Triggers.PAK")?
        .into_iter()
        .enumerate()
        .filter_map(|(slot, t)| {
            t.map(|t| {
                Ok(Trigger {
                    slot: u32::try_from(slot)?,
                    id: t.id,
                    target: t.target,
                })
            })
        })
        .collect::<Result<Vec<_>>>()?;
    write(&dir, TRIGGERS, &triggers)?;
    let bin_tables = convert_bin_tables(game_dir, &dir)?;
    let arts = arts(game_dir)?;
    write(&dir, ARTS, &arts)?;
    let voices = crate::voices::voices(&read_reference_file(game_dir, "Sacred.exe")?)?;
    write(&dir, VOICES, &voices)?;
    let idle_noises = crate::idle_noises::idle_noises();
    write(&dir, IDLE_NOISES, &idle_noises)?;

    Ok(Summary {
        creatures: creatures.len(),
        weapons: weapons.len(),
        items: items.len(),
        motions: motions.len(),
        triggers: triggers.len(),
        bin_tables,
        spells: arts.spells.len(),
        weapon_arts: arts.weapon_arts.len(),
        voices: voices.voices.len(),
        idle_noises: (idle_noises.hostile.len(), idle_noises.peaceful.len()),
    })
}

/// The `global.res` id of the text index `index`: the hash of the index in decimal (docs/02 §8).
fn text(index: u32) -> u32 {
    sacred_hash(index.to_string().as_bytes())
}

/// The ids of an art's name, long description and short description.
fn texts(head: &Head) -> Result<[u32; 3]> {
    let short = head
        .description
        .checked_add(SHORT_DESCRIPTION)
        .with_context(|| format!("Sacred.exe: art {}'s description index", head.id))?;
    Ok([text(head.name), text(head.description), text(short)])
}

/// The combat arts' two tables of `Sacred.exe`, which must be the manifest's: they are read at
/// the addresses of that build (docs/02 §6.5).
fn arts(game_dir: &Path) -> Result<Arts> {
    let exe = read_reference_file(game_dir, "Sacred.exe")?;
    let spells = arts::spells(&exe)
        .context("Sacred.exe")?
        .into_iter()
        .map(|s| {
            let [name, description, short_description] = texts(&s.head)?;
            Ok(Spell {
                id: s.head.id,
                name,
                description,
                short_description,
                icon: s.head.icon,
                sprites: s.head.sprites,
                launch: s.launch,
                regeneration: s.regeneration,
                effects: s.effects,
                values: s.values,
                caps: s.caps,
                element: s.element,
                classes: s.classes,
                flags: s.flags,
            })
        })
        .collect::<Result<_>>()?;
    let weapon_arts = arts::weapon_arts(&exe)
        .context("Sacred.exe")?
        .into_iter()
        .map(|a| {
            let [name, description, short_description] = texts(&a.head)?;
            Ok(WeaponArt {
                id: a.head.id,
                name,
                description,
                short_description,
                icon: a.head.icon,
                sprites: a.head.sprites,
                value_3c: a.value_3c,
                regeneration: a.regeneration,
                distance: a.distance,
                launch: a.launch,
                factor: a.factor,
                value_58: a.value_58,
                value_60: a.value_60,
                value_68: a.value_68,
                classes: a.classes,
                flags: a.flags,
            })
        })
        .collect::<Result<_>>()?;
    Ok(Arts {
        spells,
        weapon_arts,
    })
}

/// The data tables of `bin/`; returns how many files it converted.
fn convert_bin_tables(game_dir: &Path, dir: &Path) -> Result<usize> {
    let file = |path: &str| -> Result<Vec<u8>> { read(game_dir, path) };
    let b = bin_tables::balance(&file("bin/Balance.bin")?).context("bin/Balance.bin")?;
    let variables = bin_tables::BALANCE_KEYS
        .iter()
        .map(|key| -> Result<BalanceVariable> {
            let word = b
                .word(key)
                .with_context(|| format!("bin/Balance.bin: no word for {}", key.name))?;
            Ok(BalanceVariable {
                name: key.name.to_owned(),
                offset: key.offset,
                value: match key.kind {
                    bin_tables::Kind::F32 => BalanceValue::F32(f32::from_bits(word)),
                    bin_tables::Kind::I32 => {
                        BalanceValue::I32(i32::from_le_bytes(word.to_le_bytes()))
                    }
                },
            })
        })
        .collect::<Result<_>>()?;
    let tail = b.tail;
    write(
        dir,
        BALANCE,
        &Balance {
            variables,
            image: b.image,
            tail_1: tail
                .lists
                .into_iter()
                .map(|(key, records)| BalanceList { key, records })
                .collect(),
            quality_weights: tail
                .weights
                .into_iter()
                .map(|(key, weight)| QualityWeight { key, weight })
                .collect(),
            quality_passes: tail
                .qualities
                .into_iter()
                .map(|(key, value)| QualityPass { key, value })
                .collect(),
            tail_4: tail
                .rows
                .into_iter()
                .map(|(key, words)| BalanceRow { key, words })
                .collect(),
            effect_chances: tail
                .chances
                .into_iter()
                .map(|(effect, [a, b, most])| EffectChance { effect, a, b, most })
                .collect(),
            rest: tail.rest,
        },
    )?;
    let index = bin_tables::world_index(&file("bin/World.bin")?).context("bin/World.bin")?;
    write(dir, WORLD_INDEX, &index)?;
    let map = bin_tables::world_map(&file("bin/World2.bin")?).context("bin/World2.bin")?;
    write(dir, WORLD_MAP, &map)?;
    let sets = bin_tables::item_sets(&file("bin/sets.bin")?)
        .context("bin/sets.bin")?
        .into_iter()
        .enumerate()
        .filter_map(|(slot, s)| {
            s.map(|s| {
                Ok(ItemSet {
                    slot: u32::try_from(slot)?,
                    items: s.items,
                    hash: s.hash,
                    bytes: s.bytes,
                })
            })
        })
        .collect::<Result<Vec<_>>>()?;
    write(dir, ITEM_SETS, &sets)?;
    let modifiers: Vec<WeaponModifier> = bin_tables::weapon_modifiers(&file("bin/wpmod.bin")?)
        .context("bin/wpmod.bin")?
        .into_iter()
        .map(|m| WeaponModifier {
            words: m.words.to_vec(),
            entries: m.entries,
        })
        .collect();
    write(dir, WEAPON_MODIFIERS, &modifiers)?;
    let groups = bin_tables::weapon_groups(&file("bin/wea.bin")?).context("bin/wea.bin")?;
    write(dir, WEAPON_GROUPS, &groups)?;
    let switches = bin_tables::armour_switches(&file("bin/Rust.bin")?).context("bin/Rust.bin")?;
    write(dir, ARMOUR_SWITCHES, &switches)?;
    let stairs = bin_tables::rows(&file("bin/treppe.bin")?).context("bin/treppe.bin")?;
    write(dir, STAIRS, &stairs)?;
    let remap =
        bin_tables::static_remap(&file("bin/static10_18.bin")?).context("bin/static10_18.bin")?;
    write(dir, STATIC_REMAP, &remap)?;
    for (path, name) in [
        ("bin/merc.bin", MERCENARIES),
        ("bin/Addon/merc.bin", MERCENARIES_ADDON),
    ] {
        let rows = bin_tables::rows(&file(path)?).context(path)?;
        let mercenaries = rows
            .into_iter()
            .map(|[zero, x, y, kind]| {
                anyhow::ensure!(zero == 0, "{path}: a row does not start with 0");
                Ok(Mercenary { x, y, kind })
            })
            .collect::<Result<Vec<_>>>()?;
        write(dir, name, &mercenaries)?;
    }
    let points = bin_tables::rows(&file("bin/MultiStart.bin")?)
        .context("bin/MultiStart.bin")?
        .into_iter()
        .map(|[zero, x, y, last]| {
            anyhow::ensure!(
                zero == 0 && last == 0,
                "bin/MultiStart.bin: a row is not {{0, x, y, 0}}"
            );
            Ok(StartPoint { x, y })
        })
        .collect::<Result<Vec<_>>>()?;
    write(dir, START_POINTS, &points)?;
    Ok(12)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{repo_root, scratch_dir};

    /// A `balance.ron` of an earlier converter, without the tail's tables, still reads; the
    /// tables come back as they were written.
    #[test]
    fn a_balance_reads_with_or_without_its_tail() {
        let old: Balance = ron::from_str("(variables: [], image: [1], rest: [2])").unwrap();
        assert_eq!((old.image, old.rest), (vec![1], vec![2]));
        assert!(old.quality_weights.is_empty() && old.effect_chances.is_empty());
        let balance = Balance {
            quality_weights: vec![QualityWeight { key: 0, weight: 7 }],
            effect_chances: vec![EffectChance {
                effect: 868,
                a: 500.0,
                b: 502.0,
                most: 25.0,
            }],
            ..Balance::default()
        };
        let text = ron::to_string(&balance).unwrap();
        assert_eq!(ron::from_str::<Balance>(&text).unwrap(), balance);
    }

    /// Every table converts and reads back.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn every_table_converts() {
        let out = scratch_dir("tables");
        let game = repo_root().join("OriginalGame");
        let summary = convert(&game, &out).unwrap();
        assert_eq!(
            summary,
            Summary {
                creatures: 474,
                weapons: 4_883,
                items: 17_409,
                motions: 3_423,
                triggers: 1_246,
                bin_tables: 12,
                spells: 70,
                weapon_arts: 94,
                voices: 322,
                idle_noises: (311, 241),
            }
        );
        let dir = output_dir(&out);
        let read = |file: &str| fs::read_to_string(dir.join(file)).unwrap();
        let items: Vec<Item> = ron::from_str(&read(ITEMS)).unwrap();
        let overlay = items.iter().find(|i| i.slot == 3999).unwrap();
        assert_eq!(overlay.name, "Krombacher.grn");
        // The sound profiles of a sword's and a bow's types (#710).
        let sound = |slot| items.iter().find(|i| i.slot == slot).unwrap().sound();
        assert_eq!((sound(1729), sound(1732)), (500, 501));
        let weapons: Vec<Weapon> = ron::from_str(&read(WEAPONS)).unwrap();
        let original = tables::weapons(&game_file_bytes(&game, "PAK/Weapon.pak")).unwrap();
        // Floats survive the text form exactly.
        assert!(
            weapons
                .iter()
                .zip(&original)
                .all(|(w, o)| w.floats_00 == o.floats_00 && w.float_18 == o.float_18)
        );
        let motions: Vec<Motion> = ron::from_str(&read(MOTIONS)).unwrap();
        assert_eq!(motions.len(), 3_423);
        let _: Vec<Creature> = ron::from_str(&read(CREATURES)).unwrap();
        let _: Vec<Trigger> = ron::from_str(&read(TRIGGERS)).unwrap();
        // The arts come back from their text as they were read.
        let arts: Arts = ron::from_str(&read(ARTS)).unwrap();
        assert_eq!(arts, super::arts(&game).unwrap());
        fs::remove_dir_all(out).unwrap();
    }

    /// The voices by the byte table of `Sacred.exe`: 328 types have a case, six of them one that
    /// names the profile 0, and the start's creatures speak as docs/re/sound.md reads (#710).
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn the_creatures_speak_by_their_cases() {
        let game = repo_root().join("OriginalGame");
        let exe = read_reference_file(&game, "Sacred.exe").unwrap();
        let cases = opensacred_formats::voices::cases(&exe).unwrap();
        let spoken = cases
            .iter()
            .filter(|&&case| case != opensacred_formats::voices::NO_VOICE)
            .count();
        assert_eq!(spoken, 328);
        let voices = crate::voices::voices(&exe).unwrap().voices;
        assert_eq!(voices.len(), 322);
        let voice = |creature| {
            voices
                .iter()
                .find(|v| v.creature == creature)
                .map(|v| (v.profiles.clone(), v.rolls, v.wait))
        };
        // The goblin warrior, a bear, the Seraphim and the thief.
        assert_eq!(voice(76), Some((vec![2031], [4, 4, 4, 1], 5.0)));
        assert_eq!(voice(536), Some((vec![2009], [2, 2, 2, 1], 1.0)));
        assert_eq!(voice(1), Some((vec![1008, 1009], [4, 4, 4, 1], 10.0)));
        assert_eq!(voice(313), Some((vec![2023], [7, 5, 5, 1], 10.0)));
        // A rabbit has none.
        assert_eq!(voice(516), None);
    }

    /// The arts' texts are `global.res` ids: every name and short description is a string of
    /// it, and every long description but the eleven spells' of the index 0 (#505).
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn the_arts_name_their_texts_by_global_res_ids() {
        let game = repo_root().join("OriginalGame");
        let arts = arts(&game).unwrap();
        let res = read(&game, "scripts/us/global.res").unwrap();
        let res = opensacred_formats::text::GlobalRes::parse(&res).unwrap();
        let texts = arts
            .spells
            .iter()
            .map(|s| (s.id, [s.name, s.description, s.short_description]))
            .chain(
                arts.weapon_arts
                    .iter()
                    .map(|a| (a.id, [a.name, a.description, a.short_description])),
            );
        let mut missing = Vec::new();
        for (id, ids) in texts {
            for (kind, text) in ["name", "description", "short"].into_iter().zip(ids) {
                if res.get(text).is_none() {
                    missing.push((id, kind));
                }
            }
        }
        let eleven: Vec<(u16, &str)> = (43..=53).map(|id| (id, "description")).collect();
        missing.sort_unstable();
        assert_eq!(missing, eleven);
        // `GUI_SPELL_IRRITATION`: the indices 816 and 9247, the short description 9847.
        let irritation = arts.spells.iter().find(|s| s.id == 18).unwrap();
        assert_eq!(
            [
                irritation.name,
                irritation.description,
                irritation.short_description
            ],
            [b"816".as_slice(), b"9247", b"9847"].map(sacred_hash)
        );
        let attack = arts.weapon_arts.iter().find(|a| a.id == 1071).unwrap();
        assert_eq!(attack.name, sacred_hash(b"8266"));
        assert_eq!(
            (attack.factor, attack.regeneration, attack.distance),
            ([0.75, 0.05], [10.0, 5.0], 130)
        );
    }

    /// A `Sacred.exe` that is not the manifest's is refused: the arts lie at that build's
    /// addresses.
    #[test]
    fn the_arts_are_read_from_the_reference_executable_alone() {
        let game = scratch_dir("tables-arts");
        fs::write(game.join("Sacred.exe"), b"MZ").unwrap();
        let error = arts(&game).unwrap_err().to_string();
        assert!(
            error.contains("not the GOG Sacred Gold 2.28 file"),
            "{error}"
        );
        fs::remove_dir_all(game).unwrap();
    }

    fn game_file_bytes(game: &Path, path: &str) -> Vec<u8> {
        read(game, path).unwrap()
    }

    /// The drops' tables of the tail of `Balance.bin` land in `balance.ron` by their meaning, the
    /// others as words, and `GoldProLevel`'s second value by its name (docs/02 §6.3, #413); a
    /// second run writes the same bytes.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn the_drops_tables_convert_reproducibly() {
        let game = repo_root().join("OriginalGame");
        let runs = ["a", "b"].map(|run| scratch_dir(&format!("tables-balance-{run}")));
        let texts: Vec<String> = runs
            .iter()
            .map(|out| {
                convert_bin_tables(&game, out).unwrap();
                fs::read_to_string(out.join(BALANCE)).unwrap()
            })
            .collect();
        assert_eq!(texts[0], texts[1]);
        let b: Balance = ron::from_str(&texts[0]).unwrap();
        let weights: Vec<u32> = b.quality_weights.iter().map(|w| w.weight).collect();
        assert_eq!(
            weights,
            [
                7, 9, 0, 3, 5, 6, 8, 10, 12, 13, 14, 15, 15, 15, 15, 15, 300, 544
            ]
        );
        assert!(b.quality_weights.iter().map(|w| w.key).eq(0..18));
        let passes: Vec<(u32, u32)> = b.quality_passes.iter().map(|p| (p.key, p.value)).collect();
        assert_eq!(
            passes[7..],
            [
                (7, 7),
                (8, 9),
                (9, 11),
                (10, 13),
                (11, 15),
                (12, 18),
                (13, 21),
                (14, 25),
                (15, 25)
            ]
        );
        assert_eq!(
            b.effect_chances.iter().find(|c| c.effect == 868),
            Some(&EffectChance {
                effect: 868,
                a: 500.0,
                b: 502.0,
                most: 25.0
            })
        );
        assert_eq!(
            (b.i32("GoldProLevel"), b.i32("GoldProLevel[1]")),
            (Some(20), Some(2000))
        );
        assert_eq!((b.tail_1.len(), b.tail_4.len(), b.rest.len()), (33, 9, 570));
        for out in runs {
            fs::remove_dir_all(out).unwrap();
        }
    }
}
