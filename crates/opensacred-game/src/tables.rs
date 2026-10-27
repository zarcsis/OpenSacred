//! The tables of `tables/` that the start and every game's load read (`items.ron`,
//! `weapons.ron`, `creatures.ron`, `motions.ron`, `balance.ron`): each read and parsed once, when
//! first asked for, and shared through `sectors::Shared` (#830).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Result, anyhow};
use opensacred_data::tables::{
    BALANCE, Balance, CREATURES, Creature, ITEMS, Item, MOTIONS, Motion, TYPE_NAMES, TypeName,
    WEAPONS, Weapon,
};
use serde::de::DeserializeOwned;

use crate::files;

/// A table parsed once, or why it could not be.
type Parsed<T> = OnceLock<Result<T, String>>;

/// The tables, parsed when first asked for: none of their files changes while the game runs.
pub struct Tables {
    dir: PathBuf,
    items: Parsed<Vec<Item>>,
    weapons: Parsed<Vec<Weapon>>,
    creatures: Parsed<Vec<Creature>>,
    motions: Parsed<Vec<Motion>>,
    balance: Parsed<Balance>,
    type_names: Parsed<Vec<TypeName>>,
}

impl Tables {
    /// The tables of the asset directory `assets`, none read yet.
    pub fn new(assets: &Path) -> Self {
        Self {
            dir: assets.join("tables"),
            items: OnceLock::new(),
            weapons: OnceLock::new(),
            creatures: OnceLock::new(),
            motions: OnceLock::new(),
            balance: OnceLock::new(),
            type_names: OnceLock::new(),
        }
    }

    /// `items.ron`, `Items.pak`'s types.
    pub fn items(&self) -> Result<&[Item]> {
        self.parsed(&self.items, ITEMS).map(Vec::as_slice)
    }

    /// `weapons.ron`, `Weapon.pak`'s records.
    pub fn weapons(&self) -> Result<&[Weapon]> {
        self.parsed(&self.weapons, WEAPONS).map(Vec::as_slice)
    }

    /// `creatures.ron`, `Creature.pak`'s records.
    pub fn creatures(&self) -> Result<&[Creature]> {
        self.parsed(&self.creatures, CREATURES).map(Vec::as_slice)
    }

    /// `motions.ron`, `MOTIONS.PAK`'s records.
    pub fn motions(&self) -> Result<&[Motion]> {
        self.parsed(&self.motions, MOTIONS).map(Vec::as_slice)
    }

    /// `balance.ron`, the balance's variables.
    pub fn balance(&self) -> Result<&Balance> {
        self.parsed(&self.balance, BALANCE)
    }

    /// `type_names.ron`, the types' names of `Sacred.exe` (#1171).
    pub fn type_names(&self) -> Result<&[TypeName]> {
        self.parsed(&self.type_names, TYPE_NAMES).map(Vec::as_slice)
    }

    /// The path of the table `file`, for messages.
    pub fn path(&self, file: &str) -> PathBuf {
        self.dir.join(file)
    }

    /// The table `file`, read and parsed into `cell` the first time.
    fn parsed<'a, T: DeserializeOwned>(&self, cell: &'a Parsed<T>, file: &str) -> Result<&'a T> {
        cell.get_or_init(|| {
            let path = self.path(file);
            let text = files::read_to_string(&path)
                .map_err(|e| format!("reading {}: {e}", path.display()))?;
            let table =
                ron::from_str(&text).map_err(|e| format!("parsing {}: {e}", path.display()))?;
            bevy::log::debug!("tables: {file} parsed");
            Ok(table)
        })
        .as_ref()
        .map_err(|e| anyhow!("{e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_is_parsed_once_and_its_failure_kept() {
        let dir =
            std::env::temp_dir().join(format!("opensacred-game-tables-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("tables")).unwrap();
        std::fs::write(
            dir.join("tables").join(BALANCE),
            ron::to_string(&Balance::default()).unwrap(),
        )
        .unwrap();
        let tables = Tables::new(&dir);
        let first = tables.balance().unwrap();
        // The file gone, the table stays as it was parsed.
        std::fs::remove_file(dir.join("tables").join(BALANCE)).unwrap();
        assert!(std::ptr::eq(first, tables.balance().unwrap()));
        let error = tables.items().unwrap_err().to_string();
        assert!(
            error.contains("reading") && error.contains(ITEMS),
            "{error}"
        );
        std::fs::write(dir.join("tables").join(ITEMS), "[]").unwrap();
        assert!(tables.items().is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
