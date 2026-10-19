//! `tables` subcommand: `Creature.pak`, `Weapon.pak`, `Items.pak` with the overlay
//! `Items03.pak`, `MOTIONS.PAK`, `Triggers.PAK` and the data tables of `bin/` → one RON file each
//! under `<out>/tables/` (docs/02 §6; the format is [`opensacred_data::tables`]).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use opensacred_data::tables::{
    ARMOUR_SWITCHES, BALANCE, Balance, BalanceValue, BalanceVariable, CREATURES, Creature,
    ITEM_SETS, ITEMS, Item, ItemSet, MERCENARIES, MERCENARIES_ADDON, MOTIONS, Mercenary, Motion,
    STAIRS, START_POINTS, STATIC_REMAP, StartPoint, TRIGGERS, Trigger, WEAPON_GROUPS,
    WEAPON_MODIFIERS, WEAPONS, WORLD_INDEX, WORLD_MAP, Weapon, WeaponModifier,
};
use opensacred_formats::bin_tables;
use opensacred_formats::items::{self, Items};
use opensacred_formats::mdl::{Kind, MdlPak};
use opensacred_formats::tables;
use serde::Serialize;

use crate::verify::read_game_file;

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
}

fn read(game_dir: &Path, path: &str) -> Result<Vec<u8>> {
    read_game_file(&game_dir.join(path))
}

fn write<T: Serialize>(dir: &Path, file: &str, value: &T) -> Result<()> {
    // One record per line block, its arrays on one line each.
    let config = ron::ser::PrettyConfig::default().compact_arrays(true);
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

    Ok(Summary {
        creatures: creatures.len(),
        weapons: weapons.len(),
        items: items.len(),
        motions: motions.len(),
        triggers: triggers.len(),
        bin_tables,
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
    write(
        dir,
        BALANCE,
        &Balance {
            variables,
            image: b.image,
            rest: b.rest,
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
            }
        );
        let dir = output_dir(&out);
        let read = |file: &str| fs::read_to_string(dir.join(file)).unwrap();
        let items: Vec<Item> = ron::from_str(&read(ITEMS)).unwrap();
        let overlay = items.iter().find(|i| i.slot == 3999).unwrap();
        assert_eq!(overlay.name, "Krombacher.grn");
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
        fs::remove_dir_all(out).unwrap();
    }

    fn game_file_bytes(game: &Path, path: &str) -> Vec<u8> {
        read(game, path).unwrap()
    }
}
