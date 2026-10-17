//! MDL model containers: `PAK/models.pak` and `PAK/models03.pak` (docs/02 §4.1).
//!
//! A record is a descriptor (`ModelDesc` of 1194 bytes or `MotionDesc` of 256 bytes) followed by a
//! whole Granny 1.2b file. The directory lists every model (type 0x40) first, then every motion
//! (type 0x41); the first model and the first motion are the `INVALID_MODEL` and `INVALID_MOTION`
//! placeholders without data. The descriptor names the bounds of its GRN file, which starts right
//! after it.

use crate::error::{Result, field, to_usize};
use crate::grn::Grn;
use crate::pak::Pak;
use crate::reader::Reader;

pub const MAGIC: [u8; 3] = *b"MDL";
pub const VERSION: u8 = 3;
pub const MODEL_DESC_SIZE: u32 = 1194;
pub const MOTION_DESC_SIZE: u32 = 256;
pub const NAME_LEN: usize = 32;
/// Slots of the motion table at `ModelDesc +0x70`.
pub const MOTION_SLOTS: usize = 256;
/// Offset of the motion table in a `ModelDesc`.
const MOTIONS_AT: usize = 0x70;
/// End of the fields shared by both descriptors: name, 0xFF, dataOffset, dataSize, flags.
const SHARED_FIELDS_END: usize = 0x2E;

/// Record kind; its code is the directory type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// 0x40: a model with its skeleton, meshes and materials.
    Model,
    /// 0x41: a motion (animation clip); 23 of them carry meshes as well (`GLADIATOR.GRN`).
    Motion,
}

impl Kind {
    /// Directory type of the record.
    pub fn code(self) -> u32 {
        match self {
            Self::Model => 0x40,
            Self::Motion => 0x41,
        }
    }

    pub fn descriptor_size(self) -> u32 {
        match self {
            Self::Model => MODEL_DESC_SIZE,
            Self::Motion => MOTION_DESC_SIZE,
        }
    }

    /// Directory `size` of a record with `data_size` bytes of GRN data: the descriptor plus the
    /// data, which a model counts twice. Confirmed on every record; why the loader wants a model
    /// twice is not known.
    fn directory_size(self, data_size: u32) -> u64 {
        let copies = match self {
            Self::Model => 2,
            Self::Motion => 1,
        };
        u64::from(self.descriptor_size()) + copies * u64::from(data_size)
    }
}

/// MDL container, version 3.
#[derive(Debug)]
pub struct MdlPak<'a> {
    pak: Pak<'a>,
    models: usize,
}

impl<'a> MdlPak<'a> {
    /// Parses the header and directory; the header fields after the directory length are
    /// `nModels, nMotions, sizeof(ModelDesc), sizeof(MotionDesc), X` (X is 13268 in `models.pak`
    /// and 19897 in `models03.pak`, meaning unknown).
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        let pak = Pak::parse_expecting(data, MAGIC, VERSION)?;
        let &[count, models, motions, model_desc, motion_desc, ..] = pak.header_fields();
        let header = Reader::new(data);
        // Header field `i` is at 4 + 4·i: after the magic and the version byte.
        if u64::from(models) + u64::from(motions) != u64::from(count) {
            return Err(header.error_at(
                8,
                field(
                    "nModels + nMotions",
                    u64::from(models) + u64::from(motions),
                    format!("directory length {count}"),
                ),
            ));
        }
        for (at, name, found, expected) in [
            (16, "sizeof(ModelDesc)", model_desc, MODEL_DESC_SIZE),
            (20, "sizeof(MotionDesc)", motion_desc, MOTION_DESC_SIZE),
        ] {
            if found != expected {
                return Err(header.error_at(at, field(name, found.into(), expected.to_string())));
            }
        }
        Ok(Self {
            pak,
            models: to_usize(models),
        })
    }

    pub fn pak(&self) -> &Pak<'a> {
        &self.pak
    }

    /// Number of records, models and motions together.
    pub fn len(&self) -> usize {
        self.pak.entries().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Number of models: records `0..models()`; the motions follow them.
    pub fn models(&self) -> usize {
        self.models
    }

    pub fn motions(&self) -> usize {
        self.len() - self.models
    }

    /// Kind of record `index`, by its position in the directory.
    pub fn kind(&self, index: usize) -> Kind {
        if index < self.models {
            Kind::Model
        } else {
            Kind::Motion
        }
    }

    /// Record `index` with its descriptor checked against the directory.
    pub fn record(&self, index: usize) -> Result<Record<'a>> {
        let entry = self.pak.entry(index)?;
        let kind = self.kind(index);
        let mut r = self.pak.record_reader(index, entry);
        let entry_at = self.pak.entry_offset(index);
        if entry.kind != kind.code() {
            return Err(r.error_at(
                entry_at,
                field(
                    "directory type",
                    entry.kind.into(),
                    format!(
                        "{:#x} at position {index} of {} models",
                        kind.code(),
                        self.models
                    ),
                ),
            ));
        }

        let mut d = r.take(to_usize(kind.descriptor_size()), "MDL descriptor")?;
        let name = d.array("name")?; // +0x00
        // +0x20: 0xFF; the 33-character `VAMP_WEAPON_SWORDLONG_2HANDED.GRN` overwrites it with its
        // last character and NUL.
        d.u32()?;
        let data_offset_at = d.offset();
        let data_offset = d.u32()?; // +0x24
        let data_size = d.u32()?; // +0x28
        let flags = d.u16()?; // +0x2C

        // `r` stands right after the descriptor.
        if to_usize(data_offset) != r.offset() {
            return Err(r.error_at(
                data_offset_at,
                field(
                    "dataOffset",
                    data_offset.into(),
                    format!("{:#x}, the end of the descriptor", r.offset()),
                ),
            ));
        }
        if u64::from(entry.size) != kind.directory_size(data_size) {
            return Err(r.error_at(
                entry_at + 8,
                field(
                    "directory size",
                    entry.size.into(),
                    format!(
                        "{} for a {kind:?} with dataSize {data_size}",
                        kind.directory_size(data_size)
                    ),
                ),
            ));
        }

        let motions = match kind {
            Kind::Model => {
                d.take(MOTIONS_AT - SHARED_FIELDS_END, "ModelDesc fields")?;
                let mut motions = Vec::with_capacity(MOTION_SLOTS);
                for _ in 0..MOTION_SLOTS {
                    let at = d.offset();
                    let motion = d.u32()?;
                    // 0 is "none" (and the ordinal of `INVALID_MOTION`), valid in any container.
                    if motion != 0 && to_usize(motion) >= self.motions() {
                        return Err(d.error_at(
                            at,
                            field(
                                "motion ordinal",
                                motion.into(),
                                format!("below the {} motions of the container", self.motions()),
                            ),
                        ));
                    }
                    motions.push(motion);
                }
                motions
            }
            Kind::Motion => Vec::new(),
        };

        let grn = r.take(to_usize(data_size), "GRN file")?;
        Ok(Record {
            index,
            kind,
            name,
            data_offset,
            flags,
            motions,
            grn,
        })
    }
}

/// Container record: the descriptor fields that are understood, and the GRN file.
#[derive(Debug, Clone)]
pub struct Record<'a> {
    pub index: usize,
    pub kind: Kind,
    name: [u8; NAME_LEN],
    /// +0x24: offset of the GRN file from the start of the container.
    pub data_offset: u32,
    /// +0x2C: 3 for a model, 5 for a motion, 0 for the placeholders; meaning unknown.
    pub flags: u16,
    /// Models only, empty for a motion: the 256 slots at `ModelDesc +0x70`. A slot holds the
    /// ordinal of a motion among the motions of the same container (record `models() + ordinal`),
    /// 0 for none; according to openheilig the slot number is the game's motion enum.
    pub motions: Vec<u32>,
    grn: Reader<'a>,
}

impl<'a> Record<'a> {
    /// Name (`*.GRN`, any case) up to the first NUL. The 33-character
    /// `VAMP_WEAPON_SWORDLONG_2HANDED.GRN` is cut to its first 32 characters.
    pub fn name(&self) -> &[u8] {
        self.name.split(|&b| b == 0).next().unwrap_or_default()
    }

    /// The Granny 1.2b file; empty for `INVALID_MODEL` and `INVALID_MOTION`.
    pub fn grn(&self) -> &'a [u8] {
        self.grn.rest()
    }

    /// The GRN file parsed to the container level; `None` for the placeholders. Errors carry
    /// the record and offsets from the start of the container.
    pub fn parse_grn(&self) -> Result<Option<Grn<'a>>> {
        if self.grn.rest().is_empty() {
            return Ok(None);
        }
        Grn::from_reader(self.grn.clone()).map(Some)
    }
}

#[cfg(test)]
mod game_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{ErrorKind, Place};
    use crate::grn::TAG_BONE;
    use crate::pak::{DIR_ENTRY_SIZE, HEADER_SIZE};
    use crate::testing::{GrnNode, PakRecord, build_grn, build_mdl, mdl_record};

    fn model(name: &[u8], motions: &[(usize, u32)], grn: &[u8]) -> PakRecord {
        mdl_record(Kind::Model, name, 3, motions, grn)
    }

    fn motion(name: &[u8], grn: &[u8]) -> PakRecord {
        mdl_record(Kind::Motion, name, 5, &[], grn)
    }

    fn sample() -> Vec<u8> {
        build_mdl(
            vec![
                mdl_record(Kind::Model, b"INVALID_MODEL", 0, &[], &[]),
                model(b"BAT.GRN", &[(1, 2), (255, 1)], b"model bytes"),
            ],
            vec![
                mdl_record(Kind::Motion, b"INVALID_MOTION", 0, &[], &[]),
                motion(b"BAT_IDLE.GRN", b"idle"),
                motion(b"BAT_RUN.GRN", b"run!"),
            ],
        )
    }

    #[test]
    fn reads_models_then_motions() {
        let data = sample();
        let pak = MdlPak::parse(&data).unwrap();
        assert_eq!((pak.len(), pak.models(), pak.motions()), (5, 2, 3));

        let placeholder = pak.record(0).unwrap();
        assert_eq!(placeholder.name(), b"INVALID_MODEL");
        assert!(placeholder.grn().is_empty());
        assert_eq!(placeholder.motions, [0; MOTION_SLOTS]);

        let bat = pak.record(1).unwrap();
        assert_eq!(bat.kind, Kind::Model);
        assert_eq!(bat.name(), b"BAT.GRN");
        assert_eq!(bat.flags, 3);
        assert_eq!(bat.grn(), b"model bytes");
        assert_eq!(
            usize::try_from(bat.data_offset).unwrap(),
            usize::try_from(pak.pak().entries()[1].offset).unwrap() + 1194
        );
        let used: Vec<_> = (0..MOTION_SLOTS)
            .filter(|&slot| bat.motions[slot] != 0)
            .map(|slot| (slot, bat.motions[slot]))
            .collect();
        assert_eq!(used, [(1, 2), (255, 1)]);

        let run = pak.record(4).unwrap();
        assert_eq!(run.kind, Kind::Motion);
        assert_eq!(
            (run.name(), run.flags, run.grn()),
            (&b"BAT_RUN.GRN"[..], 5, &b"run!"[..])
        );
        assert!(run.motions.is_empty());
    }

    #[test]
    fn name_without_nul_fills_all_32_bytes() {
        let long = b"VAMP_WEAPON_SWORDLONG_2HANDED.GR";
        assert_eq!(long.len(), NAME_LEN);
        let mut record = model(long, &[], b"x");
        // The 33rd character and its NUL overwrite the 0xFF word, as in `models.pak`.
        record.bytes[0x20..0x24].copy_from_slice(b"N\0\0\0");
        let data = build_mdl(vec![record], vec![]);
        let pak = MdlPak::parse(&data).unwrap();
        assert_eq!(pak.record(0).unwrap().name(), long);
    }

    #[test]
    fn header_counts_must_add_up_to_the_directory() {
        let mut data = sample();
        data[12..16].copy_from_slice(&4u32.to_le_bytes()); // nMotions
        let err = MdlPak::parse(&data).unwrap_err();
        assert_eq!(err.place.offset, 8);
        assert_eq!(
            err.to_string(),
            "offset 0x8: field `nModels + nMotions` is 6, expected directory length 5"
        );
    }

    #[test]
    fn descriptor_sizes_are_checked() {
        let mut data = sample();
        data[20..24].copy_from_slice(&255u32.to_le_bytes());
        let err = MdlPak::parse(&data).unwrap_err();
        assert_eq!(err.place.offset, 20);
        assert!(matches!(
            err.kind,
            ErrorKind::Field {
                field: "sizeof(MotionDesc)",
                found: 255,
                ..
            }
        ));
    }

    #[test]
    fn a_motion_among_the_models_is_an_error() {
        let data = build_mdl(vec![motion(b"A.GRN", b"a")], vec![]);
        let err = MdlPak::parse(&data).unwrap().record(0).unwrap_err();
        assert_eq!(err.place.record, Some(0));
        assert_eq!(err.place.offset, HEADER_SIZE);
        assert!(matches!(
            err.kind,
            ErrorKind::Field {
                field: "directory type",
                found: 0x41,
                ..
            }
        ));
    }

    #[test]
    fn data_offset_must_point_past_the_descriptor() {
        let mut data = sample();
        let at = usize::try_from(MdlPak::parse(&data).unwrap().pak().entries()[3].offset).unwrap();
        data[at + 0x24] ^= 1;
        let err = MdlPak::parse(&data).unwrap().record(3).unwrap_err();
        assert_eq!(
            err.place,
            Place {
                record: Some(3),
                offset: at + 0x24
            }
        );
        assert!(matches!(
            err.kind,
            ErrorKind::Field {
                field: "dataOffset",
                ..
            }
        ));
    }

    #[test]
    fn directory_size_counts_model_data_twice() {
        let mut record = model(b"A.GRN", &[], b"abcd");
        assert_eq!(record.size, 1194 + 2 * 4);
        record.size = 1194 + 4;
        let data = build_mdl(vec![record], vec![]);
        let err = MdlPak::parse(&data).unwrap().record(0).unwrap_err();
        assert_eq!(err.place.offset, HEADER_SIZE + 8);
        assert_eq!(
            err.to_string(),
            format!(
                "record 0, offset {:#x}: field `directory size` is 1198, expected 1202 for a Model with dataSize 4",
                HEADER_SIZE + 8
            )
        );
    }

    #[test]
    fn motion_ordinal_must_name_a_motion_of_the_container() {
        let data = build_mdl(
            vec![model(b"A.GRN", &[(7, 2)], b"a")],
            vec![motion(b"INVALID_MOTION", &[]), motion(b"B.GRN", b"b")],
        );
        let pak = MdlPak::parse(&data).unwrap();
        let err = pak.record(0).unwrap_err();
        let first = HEADER_SIZE + 3 * DIR_ENTRY_SIZE;
        assert_eq!(err.place.offset, first + 0x70 + 7 * 4);
        assert!(matches!(
            err.kind,
            ErrorKind::Field {
                field: "motion ordinal",
                found: 2,
                ..
            }
        ));
    }

    #[test]
    fn grn_past_the_end_is_an_error() {
        let mut data = sample();
        data.truncate(data.len() - 1);
        let pak = MdlPak::parse(&data).unwrap();
        let last = pak.len() - 1;
        let err = pak.record(last).unwrap_err();
        assert!(matches!(
            err.kind,
            ErrorKind::OutOfBounds {
                what: "GRN file",
                len: 4,
                ..
            }
        ));
    }

    #[test]
    fn parse_grn_reads_the_record_data_in_place() {
        let file = build_grn(&[(
            crate::grn::SECTION_OBJECT,
            vec![GrnNode::new(TAG_BONE, 0, b"b")],
        )]);
        let mut corrupt = file.clone();
        *corrupt.last_mut().unwrap() ^= 1;
        let data = build_mdl(
            vec![
                mdl_record(Kind::Model, b"INVALID_MODEL", 0, &[], &[]),
                model(b"A.GRN", &[], &file),
                model(b"B.GRN", &[], &corrupt),
            ],
            vec![],
        );
        let pak = MdlPak::parse(&data).unwrap();
        assert!(pak.record(0).unwrap().parse_grn().unwrap().is_none());

        let grn = pak.record(1).unwrap().parse_grn().unwrap().unwrap();
        let bones = &grn.sections()[0].nodes()[0];
        assert_eq!((bones.tag, bones.bytes()), (TAG_BONE, b"b".as_slice()));

        // The CRC field of the only section entry, counted from the start of the container.
        let record = pak.record(2).unwrap();
        let err = record.parse_grn().unwrap_err();
        let crc_at = usize::try_from(record.data_offset).unwrap() + 0x60 + 12;
        assert_eq!(
            err.place,
            Place {
                record: Some(2),
                offset: crc_at
            }
        );
    }
}
