//! The items' load (#2865): the two passes of `Sacred.exe` that change its items table as it loads
//! it (`0x4293B0` and `0x429950`, docs/02 §6.1, "The client's changes") run over a table filled
//! from the user's `Items.pak`, the overlay `Items03.pak` laid into it where the first pass asks
//! for it, as the client lays it. The calls that reach the engine are stubs: the lookups of a
//! texture and of a model by its name answer a marker that tells the name, the others nothing.
//! What the passes left in the table that the file does not hold is the set's vectors, which
//! `opensacred-convert`'s `item_patches` is compared with. The oracle fills the table and reads
//! it; what the passes change is the original's machine code (docs/08 §3).

use std::collections::HashMap;

use anyhow::{Context, Result, bail, ensure};
use opensacred_data::texture::TEXTURE03_BASE;
use opensacred_formats::items::Items;
use serde::Serialize;

use crate::machine::{Machine, Stub, Text};

/// The first pass (a `fastcall` on the table): the hand-made changes, the overlays, and a few
/// changes after them.
const FIRST_PASS: u32 = 0x42_93B0;
/// The second pass (a `fastcall` on the table): the dummy, the waters' creatures, the ids that
/// name another type, and the models' scales.
const SECOND_PASS: u32 = 0x42_9950;
/// The getter of the texture manager (`cdecl`) and its lookup of a texture by its name (a
/// `thiscall` with the name).
const TEXTURES: u32 = 0x65_69C0;
const TEXTURE_BY_NAME: u32 = 0x65_7360;
/// The overlay's load (a `thiscall` on the table with the file's name and its number), which the
/// first pass calls for `PAK\ITEMS00.PAK` to `PAK\ITEMS15.PAK`.
const OVERLAY: u32 = 0x42_9190;
/// The getter of the model manager (`cdecl`) and its functions, each a `thiscall`: the lookup of
/// a model by its name, a model's scale (the model and three floats) and the copy of a model's
/// motions (two models).
const MODELS: u32 = 0x41_15D0;
const MODEL_BY_NAME: u32 = 0x41_1730;
const MODEL_SCALE: u32 = 0x41_34F0;
const MODEL_MOTIONS: u32 = 0x41_36D0;
/// A type's name for the log, and the client's log (both `cdecl`).
const TYPE_NAME: u32 = 0x43_CED0;
const LOG_PRINTF: u32 = 0x66_EF40;

/// The table: 32,768 records of 128 bytes.
const SLOTS: usize = 32_768;
const RECORD: usize = 128;
/// The fields of a record that the overlay's laying reads and changes (docs/02 §6.1): the
/// texture, the category, the effects' texture, the model's index and the slot the record names.
const TEXTURE: usize = 0x08;
const CATEGORY: usize = 0x2E;
const EFFECTS: usize = 0x66;
const MODEL: usize = 0x70;
const OWN_SLOT: usize = 0x76;
/// The name by which the first pass asks for the installation's one overlay.
const OVERLAY_NAME: &str = "PAK\\ITEMS03.PAK";

/// The markers tried for the lookups' answers, and how many values each spans: more than the
/// lookups of the two passes.
const MARKERS: [u32; 3] = [0xA5C3_0000, 0x5AC3_0000, 0xC35A_0000];
const MARKER_SPAN: u32 = 0x100;

/// What the load left in a type's record that the file does not hold: the bytes at an offset,
/// or the answer of a lookup by a name, stored there as a u32.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct Change {
    /// The type, its slot of the table.
    #[serde(rename = "type")]
    pub slot: u32,
    /// The offset in the type's record.
    pub offset: u32,
    /// The bytes there after the load, where they are not a lookup's answer.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bytes: Vec<u8>,
    /// `texture` or `model`, where the u32 there is what the client's lookup of `name` answers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lookup: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// The table as `Items.pak` fills it: each used slot's record, zeros in the others.
fn table(items: &Items<'_>) -> Result<Vec<u8>> {
    ensure!(
        items.len() == SLOTS,
        "{} slots, the table has {SLOTS}",
        items.len()
    );
    let mut table = vec![0; SLOTS * RECORD];
    for (slot, record) in table.as_chunks_mut::<RECORD>().0.iter_mut().enumerate() {
        if items.get(slot)?.is_some() {
            record.copy_from_slice(items.bytes(slot)?);
        }
    }
    Ok(table)
}

/// The overlay's records as the client lays them (`0x429190`, docs/re/equipment.md): each used
/// one of a category other than 0, for the slot that its own `+0x76` names, its texture and its
/// effects' texture raised by `base`, where its pak's textures start in the client's one
/// numbering, and its model's index cleared.
fn laid(overlay: &Items<'_>, base: u32) -> Result<Vec<(usize, Vec<u8>)>> {
    let mut laid = Vec::new();
    for index in 0..overlay.len() {
        if overlay.get(index)?.is_none() {
            continue;
        }
        let mut record = overlay.bytes(index)?.to_vec();
        if record[CATEGORY] == 0 {
            continue;
        }
        let slot = usize::from(u16::from_le_bytes([record[OWN_SLOT], record[OWN_SLOT + 1]]));
        ensure!(
            slot < SLOTS,
            "the record {index} names the slot {slot}, past the table"
        );
        for at in [TEXTURE, EFFECTS] {
            let value = word(&record, at).wrapping_add(base);
            record[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        record[MODEL..MODEL + 4].fill(0);
        laid.push((slot, record));
    }
    Ok(laid)
}

/// The u32 at `at` of `bytes`.
fn word(bytes: &[u8], at: usize) -> u32 {
    bytes[at..]
        .first_chunk::<4>()
        .map_or(0, |bytes| u32::from_le_bytes(*bytes))
}

/// A marker of [`MARKERS`] none of whose span's values lies anywhere in `table`, at any offset,
/// so that a value of the span found there after the load is a lookup's answer.
fn marker(table: &[u8]) -> Result<u32> {
    for marker in MARKERS {
        let free = table
            .windows(4)
            .all(|bytes| word(bytes, 0).wrapping_sub(marker) >= MARKER_SPAN);
        if free {
            return Ok(marker);
        }
    }
    bail!("the table holds values of every marker's span: {MARKERS:x?}");
}

/// The changes between the table `before` and `after` the load: a u32 of `marker`'s span is the
/// answer of the lookup at its place of `asked`, its kind and name; the other bytes that differ
/// go in runs.
fn differences(
    before: &[u8],
    after: &[u8],
    marker: u32,
    asked: &[(&'static str, String)],
) -> Result<Vec<Change>> {
    let mut changes = Vec::new();
    let (before, after) = (
        before.as_chunks::<RECORD>().0,
        after.as_chunks::<RECORD>().0,
    );
    let records = before.iter().zip(after).enumerate();
    for (slot, (old, new)) in records {
        if old == new {
            continue;
        }
        let slot = u32::try_from(slot)?;
        let mut looked = [false; RECORD];
        for offset in 0..=RECORD - 4 {
            let place = word(new, offset).wrapping_sub(marker);
            if place >= MARKER_SPAN {
                continue;
            }
            let Some((kind, name)) = asked.get(usize::try_from(place)?) else {
                bail!("the type {slot} holds a marker of no lookup at {offset:#x}");
            };
            changes.push(Change {
                slot,
                offset: u32::try_from(offset)?,
                bytes: Vec::new(),
                lookup: Some(kind),
                name: Some(name.clone()),
            });
            looked[offset..offset + 4].fill(true);
        }
        let differs = |at: usize| !looked[at] && old[at] != new[at];
        let mut offset = 0;
        while offset < RECORD {
            if !differs(offset) {
                offset += 1;
                continue;
            }
            let start = offset;
            while offset < RECORD && differs(offset) {
                offset += 1;
            }
            changes.push(Change {
                slot,
                offset: u32::try_from(start)?,
                bytes: new[start..offset].to_vec(),
                lookup: None,
                name: None,
            });
        }
    }
    changes.sort_by_key(|change| (change.slot, change.offset));
    Ok(changes)
}

/// A stub of the name `name` that answers `value` and removes `pops` bytes of arguments.
fn stub(name: &str, value: u32, pops: u32) -> Stub {
    Stub {
        name: name.to_owned(),
        value,
        pops,
    }
}

/// Runs the two passes over the table of `items`, the bytes of `Items.pak`, with the records of
/// `overlay`, the bytes of `Items03.pak`, laid as the first pass asks for them: what the passes
/// changed. The machine is the load's own: its stubs stay.
pub fn changes(machine: &mut Machine<'_>, items: &[u8], overlay: &[u8]) -> Result<Vec<Change>> {
    let items = Items::parse(items).context("PAK/Items.pak")?;
    let overlay = Items::parse(overlay).context("PAK/Items03.pak")?;
    let filled = table(&items).context("PAK/Items.pak")?;
    let laid = laid(&overlay, TEXTURE03_BASE).context("PAK/Items03.pak")?;
    // The file's table: what the load would leave if the passes changed nothing.
    let mut before = filled.clone();
    for (slot, record) in &laid {
        before[slot * RECORD..(slot + 1) * RECORD].copy_from_slice(record);
    }
    let marker = marker(&before)?;

    let address = machine.object(u32::try_from(filled.len())?)?;
    machine.write(address, &filled)?;
    let mut writes = Vec::new();
    for (slot, record) in laid {
        writes.push((address + u32::try_from(slot * RECORD)?, record));
    }
    // The managers that the getters answer are never read: their functions are stubs.
    let manager = machine.object(16)?;
    machine.stub_function(TEXTURES, stub("the texture manager", manager, 0))?;
    machine.stub_text(
        TEXTURE_BY_NAME,
        stub("a texture by its name", 0, 4),
        Text::Marker(marker),
    )?;
    machine.stub_text(
        OVERLAY,
        stub("an overlay's load", 0, 8),
        Text::Writes(HashMap::from([(OVERLAY_NAME.to_owned(), writes)])),
    )?;
    machine.stub_function(MODELS, stub("the model manager", manager, 0))?;
    machine.stub_text(
        MODEL_BY_NAME,
        stub("a model by its name", 0, 4),
        Text::Marker(marker),
    )?;
    machine.stub_function(MODEL_SCALE, stub("a model's scale", 0, 16))?;
    machine.stub_function(MODEL_MOTIONS, stub("a model's motions", 0, 8))?;
    machine.stub_function(TYPE_NAME, stub("a type's name", 0, 0))?;
    machine.stub_function(LOG_PRINTF, stub("the log", 0, 0))?;

    machine.call(FIRST_PASS, Some(address), &[], false)?;
    machine.call(SECOND_PASS, Some(address), &[], false)?;

    let asked: Vec<(u32, String)> = machine.asked().to_vec();
    ensure!(
        asked.contains(&(OVERLAY, OVERLAY_NAME.to_owned())),
        "the first pass never asked for {OVERLAY_NAME}"
    );
    let mut lookups = Vec::new();
    for (stub, name) in asked {
        let kind = match stub {
            TEXTURE_BY_NAME => "texture",
            MODEL_BY_NAME => "model",
            _ => "",
        };
        lookups.push((kind, name));
    }
    ensure!(
        u32::try_from(lookups.len())? <= MARKER_SPAN,
        "{} texts asked, the markers span {MARKER_SPAN}",
        lookups.len()
    );
    let after = machine.read(address, filled.len())?;
    let changes = differences(&before, &after, marker, &lookups)?;
    ensure!(
        changes.iter().all(|change| change.lookup != Some("")),
        "the table holds a marker of a text that no lookup asked"
    );
    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_is_a_run_of_bytes_or_a_lookups_answer() {
        let before = vec![0u8; 3 * RECORD];
        let mut after = before.clone();
        // The type 1: two bytes at 0x20, one at 0x2E; the type 2: the second lookup's answer at
        // 0x66 and a byte beside it.
        after[RECORD + 0x20] = 7;
        after[RECORD + 0x21] = 9;
        after[RECORD + 0x2E] = 0x16;
        let marker = MARKERS[0];
        after[2 * RECORD + 0x66..2 * RECORD + 0x6A].copy_from_slice(&(marker + 1).to_le_bytes());
        after[2 * RECORD + 0x6A] = 5;
        let asked = [
            ("texture", "a.tga".to_owned()),
            ("model", "b.grn".to_owned()),
        ];
        let changes = differences(&before, &after, marker, &asked).unwrap();
        let bytes = |slot, offset, bytes: &[u8]| Change {
            slot,
            offset,
            bytes: bytes.to_vec(),
            lookup: None,
            name: None,
        };
        assert_eq!(
            changes,
            [
                bytes(1, 0x20, &[7, 9]),
                bytes(1, 0x2E, &[0x16]),
                Change {
                    slot: 2,
                    offset: 0x66,
                    bytes: Vec::new(),
                    lookup: Some("model"),
                    name: Some("b.grn".to_owned()),
                },
                bytes(2, 0x6A, &[5]),
            ]
        );
        // A marker of no lookup is refused.
        after[0..4].copy_from_slice(&(marker + 2).to_le_bytes());
        assert!(differences(&before, &after, marker, &asked).is_err());
    }

    #[test]
    fn a_marker_is_one_that_the_table_does_not_hold() {
        let mut table = vec![0u8; 2 * RECORD];
        assert_eq!(marker(&table).unwrap(), MARKERS[0]);
        // A value of the first marker's span at an odd offset.
        table[5..9].copy_from_slice(&(MARKERS[0] + 0xFF).to_le_bytes());
        assert_eq!(marker(&table).unwrap(), MARKERS[1]);
    }

    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn the_load_changes_a_weapons_sound_and_asks_for_the_gloves_textures() {
        use crate::pe::Image;
        use crate::{machine, vectors};

        let game = opensacred_locate::original();
        let read = |path| vectors::checked(&game, path).unwrap_or_else(|e| panic!("{e:#}"));
        let image = Image::parse(&read("Sacred.exe")).unwrap_or_else(|e| panic!("{e:#}"));
        let control = machine::control_word(53).unwrap_or_else(|e| panic!("{e:#}"));
        let mut machine = Machine::new(&image, control).unwrap_or_else(|e| panic!("{e:#}"));
        let changes = changes(
            &mut machine,
            &read("PAK/Items.pak"),
            &read("PAK/Items03.pak"),
        )
        .unwrap_or_else(|e| panic!("{e:#}"));
        // The weapon 3133 takes the sound profile 503, the two low bytes of its `+0x24`, and
        // nothing else.
        let weapon: Vec<&Change> = changes.iter().filter(|c| c.slot == 3133).collect();
        assert_eq!(weapon.len(), 1, "{weapon:?}");
        assert_eq!(
            (weapon[0].offset, &weapon[0].bytes[..]),
            (0x24, &503_u16.to_le_bytes()[..])
        );
        // The heroes' angles are the file's already, so the load changes none.
        assert!(changes.iter().all(|change| change.slot > 6));
        // The Seraphim's first gloves take their effects' texture by its name, and the undead
        // goblins their models by theirs.
        assert!(changes.iter().any(|change| {
            change.slot == 4076 && change.offset == 0x66 && change.lookup == Some("texture")
        }));
        let models = changes.iter().filter(|c| c.lookup == Some("model"));
        assert_eq!(
            models.map(|c| (c.slot, c.offset)).collect::<Vec<_>>(),
            [(142, 0x70), (143, 0x70)]
        );
    }
}
