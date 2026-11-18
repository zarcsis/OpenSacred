//! The client's own changes to its items table as it loads it (`0x4293B0`, docs/02 §6.1, the
//! client's changes; #1136), which `tables` makes to `Items.pak`'s records before it writes
//! `items.ron`, so that the table is the client's: the heroes' angles, flag bits of some types,
//! ids, categories, the sound profiles of most of the weapons 3105 to 3174, the gloves' entries and
//! fields of a few more; and those of its second pass (`0x429950`, #2579): the dummy's fields,
//! the waters' crab's and fish's category, and the ids of the types 32 to 713 that name another.
//! Facts of the original's load, each a field of a type's record and what the client puts there;
//! not a repair of a defect. The converter carries them as its constants (docs/08 §3), and a test
//! on the game files checks them against the user's `Sacred.exe`: its load keeps its values in
//! registers, so `opensacred-oracle` runs the two passes in its emulator and records what they
//! change, which the test compares with these (#2865).

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
    // The second pass (`0x429950`): the dummy 1249 its own id, no texture and no effects'
    // texture, the category 2; the waters' crab and three fish the category `0x20`.
    match slot {
        DUMMY => out.extend([
            Change::U32(ID, DUMMY),
            Change::U32(TEXTURE, 0),
            Change::U32(EFFECTS, 0),
            Change::U8(CATEGORY, 2),
        ]),
        163..=166 => out.push(Change::U8(CATEGORY, 0x20)),
        _ => {}
    }
    out
}

/// The dummy's type.
const DUMMY: u32 = 1249;
/// The types whose id the second pass checks (`0x429950`): 32 to 713.
const OWN_IDS: std::ops::RangeInclusive<u32> = 32..=713;

/// The second pass's ids (`0x429950`): a type of [`OWN_IDS`] whose id `+0x20` is neither 0 nor
/// the type itself takes its type, which the client logs ("hat Resid"); whether it changed.
fn own_id(slot: u32, record: &mut [u8]) -> bool {
    let id = record
        .get(ID..ID + 4)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map_or(0, u32::from_le_bytes);
    if !OWN_IDS.contains(&slot) || id == 0 || id == slot {
        return false;
    }
    put(record, ID, &slot.to_le_bytes());
    true
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
    changes.len() + usize::from(own_id(slot, record))
}

/// The offsets of the textures and of the model's index of a record.
const TEXTURE: usize = 0x08;
const MODEL: usize = 0x70;

/// Makes the client's changes to a record of the overlay `Items03.pak`, its 128 bytes `record`, as
/// it loads it over its table (`0x429190`): its texture `+0x08` and its effects' texture `+0x66`
/// raised by `base`, where the overlay's textures start in its one numbering, and its model's index
/// `+0x70` cleared.
pub fn overlay(record: &mut [u8], base: u32) {
    for at in [TEXTURE, EFFECTS] {
        let value = record
            .get(at..at + 4)
            .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
            .map_or(0, u32::from_le_bytes);
        put(record, at, &value.wrapping_add(base).to_le_bytes());
    }
    put(record, MODEL, &0u32.to_le_bytes());
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
    fn an_overlays_textures_follow_the_bases() {
        let mut record = [0u8; 128];
        record[TEXTURE] = 1;
        record[EFFECTS] = 2;
        record[MODEL] = 9;
        overlay(&mut record, 25_535);
        assert_eq!(&record[TEXTURE..TEXTURE + 4], &25_536u32.to_le_bytes());
        assert_eq!(&record[EFFECTS..EFFECTS + 4], &25_537u32.to_le_bytes());
        assert_eq!(record[MODEL], 0);
    }

    #[test]
    fn the_client_changes_its_types_and_no_others() {
        let changed = changed();
        assert_eq!(changed.len(), 116, "{changed:?}");
        assert!(changed.contains(&3105) && changed.contains(&3174) && !changed.contains(&3175));
        assert!(changed.contains(&DUMMY) && changed.contains(&163) && changed.contains(&166));
    }

    #[test]
    fn the_second_pass_gives_a_type_its_own_id_where_it_names_another() {
        let id = |record: &[u8]| u32::from_le_bytes(record[ID..ID + 4].try_into().unwrap());
        // 351 names 251 on file, and takes its own.
        let mut record = [0u8; 128];
        record[ID..ID + 4].copy_from_slice(&251u32.to_le_bytes());
        assert_eq!(apply(351, &mut record), 1);
        assert_eq!(id(&record), 351);
        // Its own id or none stays, as does any id outside 32 to 713.
        assert_eq!(apply(351, &mut record), 0);
        let mut none = [0u8; 128];
        assert_eq!(apply(400, &mut none), 0);
        assert_eq!(id(&none), 0);
        for slot in [31, 714] {
            let mut other = [0u8; 128];
            other[ID..ID + 4].copy_from_slice(&9u32.to_le_bytes());
            apply(slot, &mut other);
            assert_eq!(id(&other), 9, "{slot}");
        }
        let mut first = [0u8; 128];
        first[ID..ID + 4].copy_from_slice(&9u32.to_le_bytes());
        apply(32, &mut first);
        assert_eq!(id(&first), 32);
        let mut last = [0u8; 128];
        last[ID..ID + 4].copy_from_slice(&9u32.to_le_bytes());
        apply(713, &mut last);
        assert_eq!(id(&last), 713);
        // The dummy: its id, no textures, the category 2.
        let mut dummy = [0u8; 128];
        dummy[TEXTURE..TEXTURE + 4].copy_from_slice(&7459u32.to_le_bytes());
        dummy[EFFECTS] = 5;
        dummy[CATEGORY] = 3;
        assert_eq!(apply(DUMMY, &mut dummy), 4);
        assert_eq!(
            (id(&dummy), dummy[TEXTURE], dummy[EFFECTS], dummy[CATEGORY]),
            (DUMMY, 0, 0, 2)
        );
        let mut fish = [0u8; 128];
        fish[CATEGORY] = 3;
        apply(165, &mut fish);
        assert_eq!(fish[CATEGORY], 0x20);
    }

    /// A change of the load as `opensacred-oracle` records it in `items_load.json`: a type's
    /// bytes at an offset, or the lookup by a name whose answer the load stores there.
    #[derive(serde::Deserialize)]
    struct Recorded {
        #[serde(rename = "type")]
        slot: usize,
        offset: usize,
        #[serde(default)]
        bytes: Vec<u8>,
        lookup: Option<String>,
        name: Option<String>,
    }

    /// The set of the load's changes as the oracle writes it.
    #[derive(serde::Deserialize)]
    struct Set {
        sacred_exe: String,
        vectors: Vec<Recorded>,
    }

    /// The load's changes that the oracle recorded from the manifest's `Sacred.exe`, in the
    /// vectors' directory as the oracle chooses it.
    fn recorded() -> Vec<Recorded> {
        let absolute = |name: &str| {
            std::env::var_os(name)
                .map(std::path::PathBuf::from)
                .filter(|path| path.is_absolute())
        };
        let directory = absolute("OPENSACRED_VECTORS").unwrap_or_else(|| {
            let base = if cfg!(windows) {
                absolute("LOCALAPPDATA")
            } else {
                absolute("XDG_DATA_HOME")
                    .or_else(|| absolute("HOME").map(|home| home.join(".local/share")))
            };
            base.unwrap_or_else(|| panic!("no directory for the vectors: set OPENSACRED_VECTORS"))
                .join("OpenSacred")
                .join("vectors")
        });
        let file = directory.join("items_load.json");
        let text = std::fs::read_to_string(&file).unwrap_or_else(|error| {
            panic!(
                "{}: {error}; record the vectors first: cargo run --release -p \
                 opensacred-oracle --features emulator (docs/03 §3.4)",
                file.display()
            )
        });
        let set: Set = serde_json::from_str(&text)
            .unwrap_or_else(|error| panic!("{}: {error}", file.display()));
        let exe = crate::verify::GOG_MANIFEST
            .lines()
            .find_map(|line| line.strip_suffix("  Sacred.exe"))
            .unwrap();
        assert_eq!(
            set.sacred_exe,
            exe,
            "{}: not of the manifest's Sacred.exe",
            file.display()
        );
        set.vectors
    }

    /// The changes are those that the two passes of the load make in the user's `Sacred.exe`
    /// (`0x4293B0`, `0x429950`), which `opensacred-oracle` runs in its emulator over the table of
    /// `Items.pak` with the overlay laid and records (#2865): every type's record with `apply`'s
    /// changes is the file's with the recorded ones. A texture that the load looks up by its name
    /// is that name's record of `texture.pak`; a model it looks up is the record's own, whose
    /// index `+0x70`, a cache of that lookup, the converter leaves as the file has it.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn the_changes_are_those_the_executables_load_makes() {
        use opensacred_formats::items::Items;
        use opensacred_formats::tex::TexPak;

        const NAME: usize = 0x37;
        let game = opensacred_locate::original();
        let read = |path: &str| std::fs::read(game.join(path)).unwrap();
        let (base_data, overlay_data) = (read("PAK/Items.pak"), read("PAK/Items03.pak"));
        let base = Items::parse(&base_data).unwrap();
        let overlaid = Items::parse(&overlay_data).unwrap();
        let texture_data = read("PAK/texture.pak");
        let textures = TexPak::parse(&texture_data).unwrap();
        // The records of `texture.pak` of a name, whatever its case.
        let named = |name: &str| -> Vec<u32> {
            (0..textures.len())
                .filter(|&index| {
                    textures.record(index).unwrap().is_some_and(|record| {
                        record.header.name().eq_ignore_ascii_case(name.as_bytes())
                    })
                })
                .map(|index| u32::try_from(index).unwrap())
                .collect()
        };
        let u32_at =
            |record: &[u8], at: usize| u32::from_le_bytes(record[at..at + 4].try_into().unwrap());

        let recorded = recorded();
        let mut checked = 0;
        for slot in 0..base.len() {
            // The file's record as `tables` takes it, and ours with the changes.
            let mut file = vec![0u8; 128];
            let mut ours = file.clone();
            if base.get(slot).unwrap().is_some() {
                file.copy_from_slice(base.bytes(slot).unwrap());
                ours.copy_from_slice(&file);
                apply(u32::try_from(slot).unwrap(), &mut ours);
            } else if slot < overlaid.len() && overlaid.get(slot).unwrap().is_some() {
                file.copy_from_slice(overlaid.bytes(slot).unwrap());
                overlay(&mut file, opensacred_data::texture::TEXTURE03_BASE);
                ours.copy_from_slice(&file);
            }
            // The load's record: the file's with the recorded changes.
            let mut theirs = file.clone();
            for change in recorded.iter().filter(|change| change.slot == slot) {
                let at = change.offset;
                match (change.lookup.as_deref(), change.name.as_deref()) {
                    (None, _) => theirs[at..at + change.bytes.len()].copy_from_slice(&change.bytes),
                    (Some("texture"), Some(name)) => {
                        let records = named(name);
                        assert_eq!(records.len(), 1, "type {slot}: {name} is {records:?}");
                        theirs[at..at + 4].copy_from_slice(&records[0].to_le_bytes());
                    }
                    (Some("model"), Some(name)) => {
                        let own = &file[NAME..NAME + 32];
                        let own = own.split(|&byte| byte == 0).next().unwrap();
                        assert!(
                            at == MODEL && own.eq_ignore_ascii_case(name.as_bytes()),
                            "type {slot}: the model {name} looked up for {at:#x}"
                        );
                    }
                    other => panic!("type {slot}: a lookup {other:?}"),
                }
                checked += 1;
            }
            if ours != theirs {
                let at = (0..128).find(|&at| ours[at] != theirs[at]).unwrap();
                panic!(
                    "type {slot}: at {at:#x} the converter has {:#010x}, the load {:#010x}",
                    u32_at(&ours, at.min(124)),
                    u32_at(&theirs, at.min(124))
                );
            }
        }
        assert_eq!(checked, recorded.len(), "a change of a type past the table");
        assert!(checked > 90, "{checked} changes");
    }
}
