//! Sound (docs/02 §10.1–10.2): `PAK/sound.pak` (`"SND"` v1) holds the samples, RIFF WAV or MP3,
//! by id; `PAK/sndProfiles.pak` (`"SPF"` v1) the profiles that name them; a table in `Sacred.exe`
//! names the ids. Confirmed on every record by `tools/analysis/formats/sound_dump.py`.

use crate::error::{Error, ErrorKind, Place, Result, field, to_usize};
use crate::pak::Pak;
use crate::reader::Reader;

pub const SOUND_MAGIC: [u8; 3] = *b"SND";
pub const PROFILES_MAGIC: [u8; 3] = *b"SPF";
/// Directory flags of a WAV and of an MP3 record.
const WAV: u32 = 32;
const MP3: u32 = 33;
/// Most zero bytes before the first MP3 frame in the game: one frame's worth.
const MAX_MP3_LEAD: usize = 417;
const PROFILE_SIZE: usize = 184;
/// A profile's groups of sample slots.
pub const GROUPS: usize = 8;
const SLOTS: usize = 8;
/// The first record of the table of sound names in `Sacred.exe`: id 0, `SOUND_FX_INVALID`.
const FIRST_NAME: &[u8] = b"\0\0\0\0SOUND_FX_INVALID\0";
const NAME_RECORD: usize = 68;
const NAME_PREFIX: &[u8] = b"SOUND_FX_";

/// `sound.pak`: 50,000 slots, 6,598 used; a sample's id is its slot.
#[derive(Debug)]
pub struct SoundPak<'a> {
    pak: Pak<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sample<'a> {
    /// A whole RIFF WAV file.
    Wav { data: &'a [u8], format: WavFormat },
    /// MPEG-1 Layer III frames as stored: `lead` zero bytes (0, 208 or 417 in the game) before the
    /// first frame.
    Mp3 { data: &'a [u8], lead: usize },
}

/// The `fmt ` chunk of a WAV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WavFormat {
    /// 1 PCM (1,825 samples), 0x11 IMA ADPCM (1,369).
    pub tag: u16,
    pub channels: u16,
    pub rate: u32,
    pub block_align: u16,
    pub bits: u16,
}

impl<'a> SoundPak<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        Ok(Self {
            pak: Pak::parse_expecting(data, SOUND_MAGIC, 1)?,
        })
    }

    /// Number of slots.
    pub fn len(&self) -> usize {
        self.pak.entries().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The ids of the used slots, in order.
    pub fn ids(&self) -> impl Iterator<Item = usize> + '_ {
        self.pak
            .entries()
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.is_empty())
            .map(|(id, _)| id)
    }

    /// Record `id` as stored; empty for an unused slot.
    pub fn bytes(&self, id: usize) -> Result<&'a [u8]> {
        let entry = self.pak.entry(id)?;
        if entry.is_empty() {
            return Ok(&[]);
        }
        self.pak
            .record_reader(id, entry)
            .bytes(to_usize(entry.size), "sample")
    }

    /// Sample `id`; `None` for an unused slot.
    pub fn get(&self, id: usize) -> Result<Option<Sample<'a>>> {
        let entry = self.pak.entry(id)?;
        if entry.is_empty() {
            return Ok(None);
        }
        let data = self.bytes(id)?;
        let r = Reader::new(data).with_record(id);
        match entry.kind {
            WAV => Ok(Some(Sample::Wav {
                data,
                format: wav_format(&r)?,
            })),
            MP3 => {
                let lead = data.iter().take_while(|&&b| b == 0).count();
                let frame = data.get(lead..lead + 2);
                if lead > MAX_MP3_LEAD
                    || !frame.is_some_and(|f| f[0] == 0xFF && f[1] & 0xFE == 0xFA)
                {
                    return Err(r.error(ErrorKind::Signature {
                        what: "MPEG-1 Layer III frame",
                    }));
                }
                Ok(Some(Sample::Mp3 { data, lead }))
            }
            other => Err(Reader::new(self.pak.data()).error_at(
                self.pak.entry_offset(id),
                field("sample flags", other.into(), "32 (WAV) or 33 (MP3)".into()),
            )),
        }
    }
}

/// The `fmt ` chunk of the RIFF WAV `r`, after checking that the chunks fill the file and that a
/// `data` chunk is there.
fn wav_format(r: &Reader) -> Result<WavFormat> {
    let data = r.rest();
    let mut h = r.clone();
    let riff: [u8; 4] = h.array("RIFF")?;
    let size = h.u32()?;
    let wave: [u8; 4] = h.array("WAVE")?;
    if &riff != b"RIFF" || &wave != b"WAVE" {
        return Err(r.error(ErrorKind::Signature { what: "RIFF WAVE" }));
    }
    if u64::from(size) + 8 != u64::try_from(data.len()).unwrap_or(u64::MAX) {
        return Err(r.error_at(
            r.offset() + 4,
            field(
                "RIFF size",
                size.into(),
                format!("{}, the record size - 8", data.len() - 8),
            ),
        ));
    }
    let mut format = None;
    let mut has_data = false;
    while !h.rest().is_empty() {
        let id: [u8; 4] = h.array("WAV chunk")?;
        let len = to_usize(h.u32()?);
        let mut chunk = h.take(len, "WAV chunk")?;
        if len % 2 == 1 && !h.rest().is_empty() {
            h.u8()?;
        }
        match &id {
            b"fmt " => {
                let tag = chunk.u16()?;
                let channels = chunk.u16()?;
                let rate = chunk.u32()?;
                chunk.u32()?; // bytes per second
                let block_align = chunk.u16()?;
                let bits = chunk.u16()?;
                format = Some(WavFormat {
                    tag,
                    channels,
                    rate,
                    block_align,
                    bits,
                });
            }
            b"data" => has_data = true,
            _ => {}
        }
    }
    let format = format.ok_or_else(|| {
        r.error(ErrorKind::Signature {
            what: "WAV fmt chunk",
        })
    })?;
    if !has_data {
        return Err(r.error(ErrorKind::Signature {
            what: "WAV data chunk",
        }));
    }
    if !matches!(format.tag, 1 | 0x11) {
        return Err(r.error(field(
            "WAV format tag",
            format.tag.into(),
            "1 (PCM) or 0x11 (IMA ADPCM)".into(),
        )));
    }
    Ok(format)
}

/// `sndProfiles.pak`: 8,192 slots of 184 bytes, 175 profiles.
#[derive(Debug)]
pub struct Profiles<'a> {
    pak: Pak<'a>,
}

/// A sound profile: the sample variants of each of its 8 groups (an event of the profile's
/// owner: footsteps on grass, an attack, a death, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile<'a> {
    /// `music`, `footsteps_hero_male`, `weapon_bow`, … (Latin-1).
    pub name: &'a [u8],
    /// Each group's variants; empty for a group without sound. A group stores its variants
    /// repeated in turn over 8 slots.
    pub groups: [Vec<u16>; GROUPS],
}

impl<'a> Profiles<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        Ok(Self {
            pak: Pak::parse_expecting(data, PROFILES_MAGIC, 1)?,
        })
    }

    pub fn len(&self) -> usize {
        self.pak.entries().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Profile `slot`: `char name[32]`, `u32` 1, 10 zero `u16`, 8 groups of 8 `u16` sample ids;
    /// `None` for a slot without a name.
    pub fn get(&self, slot: usize) -> Result<Option<Profile<'a>>> {
        let entry = self.pak.entry(slot)?;
        let mut r = self
            .pak
            .record_reader(slot, entry)
            .take(PROFILE_SIZE, "profile")?;
        let name = r.bytes(32, "profile name")?;
        let name = name.split(|&b| b == 0).next().unwrap_or_default();
        if name.is_empty() {
            return Ok(None);
        }
        let one_at = r.offset();
        let one = r.u32()?;
        if one != 1 {
            return Err(r.error_at(one_at, field("profile +0x20", one.into(), "1".into())));
        }
        for _ in 0..10 {
            let at = r.offset();
            let zero = r.u16()?;
            if zero != 0 {
                return Err(r.error_at(at, field("profile prefix", zero.into(), "0".into())));
            }
        }
        let mut groups: [Vec<u16>; GROUPS] = Default::default();
        for group in &mut groups {
            let at = r.offset();
            let mut slots = [0u16; SLOTS];
            for slot in &mut slots {
                *slot = r.u16()?;
            }
            *group = variants(&slots).ok_or_else(|| {
                r.error_at(
                    at,
                    field("profile group", 0, "variants repeated in turn".into()),
                )
            })?;
        }
        Ok(Some(Profile { name, groups }))
    }
}

/// The variants of a group: the shortest prefix that its slots repeat in turn; none for a group
/// of zeros; `None` if the slots repeat no prefix.
fn variants(slots: &[u16; SLOTS]) -> Option<Vec<u16>> {
    if slots.iter().all(|&s| s == 0) {
        return Some(Vec::new());
    }
    (1..=SLOTS)
        .find(|&n| !slots[..n].contains(&0) && (0..SLOTS).all(|k| slots[k] == slots[k % n]))
        .map(|n| slots[..n].to_vec())
}

/// A sound id's name in `Sacred.exe`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Name<'a> {
    pub id: u32,
    /// `SOUND_FX_ANIMAL_BAT01`, `SOUND_FX_MUSIC_WOOD01`, ….
    pub name: &'a str,
}

/// The names of the sound ids, from their table in `Sacred.exe` (VA `0x964870` in GOG 2.28):
/// records of `{u32 id; char name[64]}`, zeros after the name, from `SOUND_FX_INVALID` (id 0) to
/// the last whose name starts with `SOUND_FX_`; 6,869 names, two ids with two names each. They
/// name every sample of `sound.pak` and, by the file names, the music (docs/02 §10.2).
pub fn names(exe: &[u8]) -> Result<Vec<Name<'_>>> {
    let not_found = || Error {
        place: Place {
            record: None,
            offset: 0,
        },
        kind: ErrorKind::Signature {
            what: "table of sound names",
        },
    };
    let start = exe
        .windows(FIRST_NAME.len())
        .position(|w| w == FIRST_NAME)
        .ok_or_else(not_found)?;
    let mut r = Reader::at(exe, start);
    let mut names = Vec::new();
    while r
        .rest()
        .get(4..)
        .is_some_and(|rest| rest.starts_with(NAME_PREFIX))
    {
        let id = r.u32()?;
        let at = r.offset();
        let bytes = r.bytes(NAME_RECORD - 4, "sound name")?;
        let (name, rest) =
            bytes.split_at(bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len()));
        let name = std::str::from_utf8(name)
            .ok()
            .filter(|n| n.is_ascii() && rest.iter().all(|&b| b == 0))
            .ok_or_else(|| {
                r.error_at(at, field("sound name", id.into(), "ASCII and zeros".into()))
            })?;
        names.push(Name { id, name });
    }
    Ok(names)
}

#[cfg(test)]
mod game_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{PakRecord, build_pak};

    fn wav(tag: u16) -> Vec<u8> {
        let mut fmt = Vec::new();
        for v in [tag, 1] {
            fmt.extend_from_slice(&v.to_le_bytes());
        }
        fmt.extend_from_slice(&44_100u32.to_le_bytes());
        fmt.extend_from_slice(&88_200u32.to_le_bytes());
        fmt.extend_from_slice(&2u16.to_le_bytes());
        fmt.extend_from_slice(&16u16.to_le_bytes());
        let mut body = b"WAVEfmt ".to_vec();
        body.extend_from_slice(&16u32.to_le_bytes());
        body.extend(fmt);
        body.extend_from_slice(b"data");
        body.extend_from_slice(&3u32.to_le_bytes());
        body.extend_from_slice(&[1, 2, 3, 0]);
        let mut out = b"RIFF".to_vec();
        out.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
        out.extend(body);
        out
    }

    fn mp3() -> Vec<u8> {
        let mut mp3 = vec![0; 208];
        mp3.extend_from_slice(&[0xFF, 0xFB, 0x50, 0xC4]);
        mp3
    }

    fn sound() -> Vec<u8> {
        let mp3 = mp3();
        build_pak(
            SOUND_MAGIC,
            1,
            &[
                None,
                Some(PakRecord::new(WAV, &wav(0x11))),
                Some(PakRecord::new(MP3, &mp3)),
            ],
        )
    }

    #[test]
    fn reads_wav_and_mp3_samples() {
        let data = sound();
        let pak = SoundPak::parse(&data).unwrap();
        assert_eq!(pak.ids().collect::<Vec<_>>(), [1, 2]);
        assert_eq!(pak.get(0).unwrap(), None);
        let Some(Sample::Wav { data, format }) = pak.get(1).unwrap() else {
            panic!("not a WAV");
        };
        assert_eq!(data, wav(0x11));
        assert_eq!(
            format,
            WavFormat {
                tag: 0x11,
                channels: 1,
                rate: 44_100,
                block_align: 2,
                bits: 16
            }
        );
        // The record as stored, with the count of the zeros before the first frame.
        assert_eq!(
            pak.get(2).unwrap(),
            Some(Sample::Mp3 {
                data: &mp3(),
                lead: 208
            })
        );
    }

    #[test]
    fn a_wav_must_be_whole() {
        let mut broken = wav(1);
        broken[4] += 1;
        let data = build_pak(SOUND_MAGIC, 1, &[Some(PakRecord::new(WAV, &broken))]);
        let err = SoundPak::parse(&data).unwrap().get(0).unwrap_err();
        assert!(err.to_string().contains("field `RIFF size`"), "{err}");
        let data = build_pak(SOUND_MAGIC, 1, &[Some(PakRecord::new(WAV, &wav(2)))]);
        let err = SoundPak::parse(&data).unwrap().get(0).unwrap_err();
        assert!(
            err.to_string().contains("field `WAV format tag` is 2"),
            "{err}"
        );
    }

    fn profile(name: &[u8], groups: [[u16; SLOTS]; GROUPS]) -> Vec<u8> {
        let mut r = name.to_vec();
        r.resize(32, 0);
        r.extend_from_slice(&1u32.to_le_bytes());
        r.extend_from_slice(&[0; 20]);
        for v in groups.iter().flatten() {
            r.extend_from_slice(&v.to_le_bytes());
        }
        r
    }

    #[test]
    fn profiles_give_each_group_its_variants() {
        let mut groups = [[0; SLOTS]; GROUPS];
        groups[1] = [1020, 1021, 1022, 1020, 1021, 1022, 1020, 1021];
        groups[2] = [1010; SLOTS];
        let data = build_pak(
            PROFILES_MAGIC,
            1,
            &[
                Some(PakRecord::new(35, &[0; PROFILE_SIZE])),
                Some(PakRecord::new(35, &profile(b"footsteps_hero_male", groups))),
            ],
        );
        let profiles = Profiles::parse(&data).unwrap();
        assert_eq!(profiles.get(0).unwrap(), None);
        let footsteps = profiles.get(1).unwrap().unwrap();
        assert_eq!(footsteps.name, b"footsteps_hero_male");
        assert_eq!(footsteps.groups[0], []);
        assert_eq!(footsteps.groups[1], [1020, 1021, 1022]);
        assert_eq!(footsteps.groups[2], [1010]);

        // A zero among the variants repeats no prefix.
        groups[3] = [5, 0, 0, 0, 0, 0, 0, 7];
        let data = build_pak(
            PROFILES_MAGIC,
            1,
            &[Some(PakRecord::new(35, &profile(b"odd", groups)))],
        );
        assert!(Profiles::parse(&data).unwrap().get(0).is_err());
    }
}
