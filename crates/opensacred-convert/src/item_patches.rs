//! The client's own changes to its items table as it loads it (`0x4293B0`, docs/02 §6.1, "The
//! client's changes"; #1136), which `tables` makes to `Items.pak`'s records before it writes
//! `items.ron`, so that the table is the client's: the heroes' angles, flag bits of some types,
//! ids, categories, the sound profiles of most of the weapons 3105 to 3174, the gloves' entries and
//! fields of a few more. Facts of the original's load, each a field of a type's record and what
//! the client puts there; not a repair of a defect.

/// What a change does to a field of a record, at its offset.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Change {
    /// The float there.
    F32(usize, f32),
    /// The u32 there.
    U32(usize, u32),
    /// The u8 there.
    U8(usize, u8),
    /// The u16 there, where it is 0.
    U16IfZero(usize, u16),
    /// The u16 there.
    U16(usize, u16),
    /// The bits of the u32 at `+0x00` set.
    Set(u32),
    /// The bits of the u32 at `+0x00` cleared.
    Clear(u32),
}

/// The fields' offsets in a record of 128 bytes.
const FLAGS: usize = 0x00;
const EFFECTS: usize = 0x66;
const ANGLE: usize = 0x57;
const ID: usize = 0x20;
const SOUND: usize = 0x24;
const CATEGORY: usize = 0x2E;
const FIELD_1C: usize = 0x1C;
const FIELD_61: usize = 0x61;
const FIELD_62: usize = 0x62;
const FIELD_6A: usize = 0x6A;

/// The flag of `+0x00` that the client clears or sets for a few types.
const FLAG_26: u32 = 0x0400_0000;
/// The flags word of the gloves the client sets: the kind 2, the class 5 and the subclass 8.
const GLOVES: u32 = 0x0008_0502;
/// The textures of `texture.pak` that the client gives the Seraphim's two gloves' effects by name
/// (`fx_multiTexBase04` and `fx_multiTexBase05`).
const SERAPHIM_GLOVES_EFFECTS: [u32; 2] = [197, 198];

/// The weapons' sound profiles that the client sets, each for the types it names (a slot of
/// `sndProfiles.pak`, docs/02 §10.1), and those it clears to 0.
const SOUNDS: [(u32, &[u32]); 10] = [
    (
        500,
        &[
            3112, 3118, 3119, 3120, 3123, 3124, 3125, 3126, 3132, 3138, 3150, 3151, 3152, 3153,
            3154, 3155, 3156, 3157, 3167, 3168, 3171, 3172,
        ],
    ),
    (503, &[3133]),
    (505, &[3115, 3116, 3117]),
    (506, &[3114]),
    (507, &[3127, 3129, 3130, 3137, 3158, 3159]),
    (508, &[3134, 3139, 3169, 3170, 3173, 3174]),
    (509, &[3121, 3122, 3128, 3141, 3142, 3143, 3149]),
    (516, &[3131, 3165, 3166]),
    (517, &[3135, 3136, 3140]),
    (0, &[3105, 3106, 3107, 3108, 3109, 3110, 3111, 3113]),
];

/// The changes of the type `slot`'s record, in the client's order.
fn changes(slot: u32) -> Vec<Change> {
    let mut out = Vec::new();
    match slot {
        // The heroes' angle, 90° but the Wood Elf's 0.
        1..=4 | 6 => out.push(Change::F32(ANGLE, 90.0)),
        5 => out.push(Change::F32(ANGLE, 0.0)),
        _ => {}
    }
    if matches!(slot, 536..=538 | 543 | 545 | 591 | 600) {
        out.push(Change::Clear(FLAG_26));
    }
    if matches!(slot, 802 | 1338..=1341) {
        out.push(Change::Set(FLAG_26));
    }
    // Their own slots as their ids.
    if matches!(slot, 58 | 135 | 573) {
        out.push(Change::U32(ID, slot));
    }
    // The Gladiator's three gloves and the Seraphim's two: their flags, ids and categories, the
    // Seraphim's their effects' textures.
    match slot {
        4073..=4075 => out.extend([
            Change::U8(CATEGORY, 24),
            Change::U32(FLAGS, GLOVES),
            Change::U32(ID, slot),
        ]),
        4076 => out.extend([
            Change::U8(CATEGORY, 22),
            Change::U32(FLAGS, GLOVES),
            Change::U32(ID, slot),
            Change::U32(EFFECTS, SERAPHIM_GLOVES_EFFECTS[0]),
        ]),
        4082 => out.extend([
            Change::U8(CATEGORY, 22),
            Change::U32(FLAGS, GLOVES),
            Change::U32(ID, slot),
            Change::U32(EFFECTS, SERAPHIM_GLOVES_EFFECTS[1]),
        ]),
        _ => {}
    }
    if matches!(slot, 4277 | 4284..=4302) {
        out.push(Change::U8(FIELD_62, 0));
    }
    for (sound, slots) in SOUNDS {
        if slots.contains(&slot) {
            out.push(Change::U32(SOUND, sound));
        }
    }
    // After the overlays.
    match slot {
        3240 => out.push(Change::U8(CATEGORY, 22)),
        4304 => out.extend([
            Change::U8(FIELD_61, 2),
            Change::U8(FIELD_62, 0),
            Change::U32(FIELD_1C, 8804),
        ]),
        5776 => out.extend([Change::U16(FIELD_6A, 0), Change::U32(FLAGS, 0x0800_0000)]),
        5635 | 5769 => out.push(Change::U16IfZero(FIELD_6A, 55)),
        _ => {}
    }
    out
}

/// Makes the client's changes to the record of the type `slot`, its 128 bytes `record`; how many
/// it made.
pub fn apply(slot: u32, record: &mut [u8]) -> usize {
    let changes = changes(slot);
    for change in &changes {
        match *change {
            Change::F32(at, value) => put(record, at, &value.to_le_bytes()),
            Change::U32(at, value) => put(record, at, &value.to_le_bytes()),
            Change::U8(at, value) => put(record, at, &[value]),
            Change::U16(at, value) => put(record, at, &value.to_le_bytes()),
            Change::U16IfZero(at, value) => {
                if record.get(at..at + 2) == Some(&[0, 0]) {
                    put(record, at, &value.to_le_bytes());
                }
            }
            Change::Set(bits) => {
                let flags = word(record) | bits;
                put(record, FLAGS, &flags.to_le_bytes());
            }
            Change::Clear(bits) => {
                let flags = word(record) & !bits;
                put(record, FLAGS, &flags.to_le_bytes());
            }
        }
    }
    changes.len()
}

/// The u32 at `+0x00`.
fn word(record: &[u8]) -> u32 {
    record
        .first_chunk::<4>()
        .map_or(0, |bytes| u32::from_le_bytes(*bytes))
}

/// `bytes` at `at`, where the record holds them.
fn put(record: &mut [u8], at: usize, bytes: &[u8]) {
    if let Some(span) = record.get_mut(at..at + bytes.len()) {
        span.copy_from_slice(bytes);
    }
}

/// The types that the client changes.
#[cfg(test)]
fn changed() -> Vec<u32> {
    (0..32_768)
        .filter(|&slot| !changes(slot).is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_puts_its_field_and_keeps_the_rest() {
        let mut record = [0u8; 128];
        record[0] = 2;
        record[3] = 0x04;
        // A hero's angle, a flag cleared and a sound.
        assert_eq!(apply(1, &mut record), 1);
        assert_eq!(&record[ANGLE..ANGLE + 4], &90.0f32.to_le_bytes());
        assert_eq!(apply(536, &mut record), 1);
        assert_eq!(record[..4], [2, 0, 0, 0]);
        assert_eq!(apply(3112, &mut record), 1);
        assert_eq!(&record[SOUND..SOUND + 4], &500u32.to_le_bytes());
        // A u16 put only where it is 0.
        record[FIELD_6A] = 7;
        apply(5635, &mut record);
        assert_eq!(record[FIELD_6A], 7);
        record[FIELD_6A] = 0;
        apply(5635, &mut record);
        assert_eq!(record[FIELD_6A], 55);
    }

    #[test]
    fn the_client_changes_its_types_and_no_others() {
        let changed = changed();
        assert_eq!(changed.len(), 111, "{changed:?}");
        assert!(changed.contains(&3105) && changed.contains(&3174) && !changed.contains(&3175));
    }
}
