//! Animation of a GRN file (docs/02 §4.2): a track per animated bone, each with curves of the
//! translation, the rotation and the scale/shear of the bone's local transform. A track names its
//! bone by transform channel, the channel the form gives the bone ([`Bone::channel`]). The record
//! layout follows the openheilig description (`formats/granny-grn.md`, CC BY-SA 4.0) and is
//! confirmed on every file by `tools/analysis/formats/grn_dump.py`; how the curves interpolate is
//! described in docs/02 §4.2.
//!
//! [`Bone::channel`]: super::Bone::channel

use super::{Section, TAG_TRACK, as_u64, one_based};
use crate::error::{ErrorKind, Result, field};
use crate::reader::Reader;

const TAG_ANIMATIONS: u32 = 0xCA5E_1205;
const TAG_ANIMATION: u32 = 0xCA5E_1200;
const TAG_TRACKS: u32 = 0xCA5E_1203;
/// Header of a split track: channel, 0, layout, three modes, three key counts, the time
/// selectors 0, 1, 2 and a zero word.
const SPLIT_HEADER: usize = 52;
/// Header of an interleaved track: channel, 0, layout.
const INTERLEAVED_HEADER: usize = 12;
/// Frame of an interleaved track: time, translation, rotation, scale/shear.
const FRAME_SIZE: usize = 68;

/// Animation tracks in file order. Every motion but four has one track per bone; the two
/// `FX_*_IDLE_BH.GRN` motions have two per bone, a split and an interleaved one. The models have
/// none, except `WINDSTRAUCH.GRN` (4) and `CITIZEN_MAL.GRN` (56 interleaved tracks without
/// frames).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Animation {
    pub tracks: Vec<Track>,
}

impl Animation {
    /// `1205 → 1200 → 1203 → 1204…`; the `1200` also holds an empty `1201`. `channels` is the
    /// number of transform channels of the file.
    pub fn parse(section: &Section, channels: usize) -> Result<Self> {
        let list = section.child(None, TAG_ANIMATIONS, "GRN animations")?;
        let animation = section.child(Some(list), TAG_ANIMATION, "GRN animations")?;
        let list = section.child(Some(animation), TAG_TRACKS, "GRN animation")?;
        let tracks = section
            .list(Some(list), TAG_TRACK, "GRN animation tracks")?
            .into_iter()
            .map(|node| Track::parse(node.data(), channels))
            .collect::<Result<_>>()?;
        Ok(Self { tracks })
    }

    /// Length in seconds: the latest key of any curve; 0 without keys. The tracks of a motion
    /// end together, except in the two `FX_*_IDLE_BH.GRN`.
    pub fn duration(&self) -> f32 {
        self.tracks
            .iter()
            .flat_map(|t| {
                [
                    t.translation.times.last(),
                    t.rotation.times.last(),
                    t.scale_shear.times.last(),
                ]
            })
            .flatten()
            .fold(0.0, |a, &b| a.max(b))
    }
}

/// Track: the curves of one bone's local transform `T · R · SS`.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// Transform channel of the bone, from 0 (the file numbers it from 1).
    pub channel: usize,
    pub layout: Layout,
    pub translation: Curve<3>,
    /// Quaternions `[x, y, z, w]`. A quadratic curve's keys are control points whose length
    /// matters: 615k of them are off unit length by more than 1e-3, and the curve is normalized
    /// after it is evaluated (docs/02 §4.2).
    pub rotation: Curve<4>,
    /// 3×3 matrices, symmetric, in file order.
    pub scale_shear: Curve<9>,
}

/// How a track stores its keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Code 1, 255,493 tracks: three curves with their own times and modes.
    Split,
    /// Code 0, 3,069 tracks: frames at a fixed rate (50, 30 or 60 per second), each with all
    /// three values; read as three linear curves with the same times.
    Interleaved,
}

/// Curve: keys at times in seconds, rising strictly from 0.
#[derive(Debug, Clone, PartialEq)]
pub struct Curve<const N: usize> {
    pub interpolation: Interpolation,
    pub times: Vec<f32>,
    pub keys: Vec<[f32; N]>,
}

/// Interpolation mode of a split curve (openheilig names: copy, linear, quadratic; mode 3,
/// cubic, never occurs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    /// Mode 0: holds a key; 1,262 scale/shear curves, whose keys are equal.
    Constant,
    /// Mode 1: straight between neighbouring keys; 171,412 scale/shear curves.
    Linear,
    /// Mode 2: a quadratic B-spline whose keys are control points (docs/02 §4.2); every
    /// translation and rotation curve and 82,815 scale/shear curves.
    Quadratic,
}

impl Interpolation {
    fn from_code(code: u32) -> Option<Self> {
        match code {
            0 => Some(Self::Constant),
            1 => Some(Self::Linear),
            2 => Some(Self::Quadratic),
            _ => None,
        }
    }

    /// Mode number as stored.
    pub fn code(self) -> u32 {
        match self {
            Self::Constant => 0,
            Self::Linear => 1,
            Self::Quadratic => 2,
        }
    }
}

impl Track {
    /// Track record: `u32` channel from 1, `u32` 0, `u32` layout, then the split header and
    /// arrays or the interleaved frames.
    fn parse(data: Reader, channels: usize) -> Result<Self> {
        let len = data.rest().len();
        let mut r = data;
        let channel = one_based(&mut r, channels, "track channel")?;
        let zero_at = r.offset();
        let zero = r.u32()?;
        if zero != 0 {
            return Err(r.error_at(zero_at, field("track +0x4", zero.into(), "0".into())));
        }
        let layout_at = r.offset();
        match r.u32()? {
            1 => Self::split(r, channel, len),
            0 => Self::interleaved(r, channel, len),
            other => Err(r.error_at(
                layout_at,
                field("track layout", other.into(), "0 or 1".into()),
            )),
        }
    }

    /// The modes, the key counts (at least 1 each), the selectors 0, 1, 2 and a zero word; then
    /// the times of the three curves, then their keys.
    fn split(mut r: Reader, channel: usize, len: usize) -> Result<Self> {
        let mut modes = [Interpolation::Constant; 3];
        for mode in &mut modes {
            let at = r.offset();
            let code = r.u32()?;
            *mode = Interpolation::from_code(code).ok_or_else(|| {
                r.error_at(at, field("interpolation mode", code.into(), "0..=2".into()))
            })?;
        }
        let mut counts = [0; 3];
        for count in &mut counts {
            let at = r.offset();
            let n = r.u32()?;
            if n == 0 {
                return Err(r.error_at(at, field("key count", 0, "at least 1".into())));
            }
            *count = n;
        }
        for expected in 0..3 {
            let at = r.offset();
            let selector = r.u32()?;
            if selector != expected {
                return Err(r.error_at(
                    at,
                    field("time selector", selector.into(), expected.to_string()),
                ));
            }
        }
        let pad_at = r.offset();
        let pad = r.u32()?;
        if pad != 0 {
            return Err(r.error_at(pad_at, field("track +0x30", pad.into(), "0".into())));
        }
        let [nt, nq, ns] = counts.map(u64::from);
        let expected = 52 + 16 * nt + 20 * nq + 40 * ns;
        if as_u64(len) != expected {
            return Err(r.error_at(
                r.offset() - SPLIT_HEADER,
                field(
                    "split track size",
                    as_u64(len),
                    format!("{expected}, 52 + 16 nt + 20 nq + 40 ns"),
                ),
            ));
        }
        // The size fits the record, so the counts fit in `usize`.
        let [nt, nq, ns] = counts.map(|n| usize::try_from(n).unwrap_or(usize::MAX));
        let times = [
            floats(&mut r, nt)?,
            floats(&mut r, nq)?,
            floats(&mut r, ns)?,
        ];
        let [translation_times, rotation_times, scale_times] = times;
        let translation = curve(&mut r, modes[0], translation_times)?;
        let rotation = curve(&mut r, modes[1], rotation_times)?;
        let scale_shear = curve(&mut r, modes[2], scale_times)?;
        Ok(Self {
            channel,
            layout: Layout::Split,
            translation,
            rotation,
            scale_shear,
        })
    }

    /// Frames of 68 bytes up to the end of the record.
    fn interleaved(mut r: Reader, channel: usize, len: usize) -> Result<Self> {
        let body = len - INTERLEAVED_HEADER;
        if !body.is_multiple_of(FRAME_SIZE) {
            return Err(r.error_at(
                r.offset() - INTERLEAVED_HEADER,
                field(
                    "interleaved track size",
                    as_u64(len),
                    "12 + 68 per frame".into(),
                ),
            ));
        }
        let n = body / FRAME_SIZE;
        let mut times = Vec::with_capacity(n);
        let mut translation = Vec::with_capacity(n);
        let mut rotation = Vec::with_capacity(n);
        let mut scale_shear = Vec::with_capacity(n);
        let start = r.clone();
        for _ in 0..n {
            times.push(r.f32()?);
            translation.push(r.f32s()?);
            rotation.push(r.f32s()?);
            scale_shear.push(r.f32s()?);
        }
        check_times(&start, &times, FRAME_SIZE)?;
        Ok(Self {
            channel,
            layout: Layout::Interleaved,
            translation: linear(times.clone(), translation),
            rotation: linear(times.clone(), rotation),
            scale_shear: linear(times, scale_shear),
        })
    }
}

fn linear<const N: usize>(times: Vec<f32>, keys: Vec<[f32; N]>) -> Curve<N> {
    Curve {
        interpolation: Interpolation::Linear,
        times,
        keys,
    }
}

/// `n` floats, checked as times.
fn floats(r: &mut Reader, n: usize) -> Result<Vec<f32>> {
    let start = r.clone();
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(r.f32()?);
    }
    check_times(&start, &out, 4)?;
    Ok(out)
}

/// The keys of a curve whose times are read.
fn curve<const N: usize>(
    r: &mut Reader,
    interpolation: Interpolation,
    times: Vec<f32>,
) -> Result<Curve<N>> {
    let mut keys = Vec::with_capacity(times.len());
    for _ in 0..times.len() {
        keys.push(r.f32s()?);
    }
    Ok(Curve {
        interpolation,
        times,
        keys,
    })
}

/// Times rise strictly from 0; `start` is where the first one is and `stride` the distance to
/// the next, for the error offset.
fn check_times(start: &Reader, times: &[f32], stride: usize) -> Result<()> {
    let mut previous = None;
    for (key, &time) in times.iter().enumerate() {
        let fits = previous.map_or(time == 0.0, |p| time > p);
        if !fits {
            let expected = previous.map_or_else(|| "0".to_owned(), |p| format!("above {p}"));
            return Err(start.error_at(
                start.offset() + key * stride,
                ErrorKind::Time {
                    key,
                    found: time,
                    expected,
                },
            ));
        }
        previous = Some(time);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grn::Grn;
    use crate::testing::{GrnBone, GrnNode, grn_node, grn_tree, object_grn, skeleton_nodes};

    const IDENTITY: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

    fn bone(name: &'static [u8], parent: u32) -> GrnBone {
        let mut transform = [0.0; 16];
        transform[6] = 1.0;
        transform[7..].copy_from_slice(&IDENTITY);
        GrnBone {
            name,
            parent,
            transform,
        }
    }

    fn le(values: &[u32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn put_floats(out: &mut Vec<u8>, values: &[f32]) {
        out.extend(values.iter().flat_map(|v| v.to_le_bytes()));
    }

    /// Split track of channel `channel` (from 1): translation keys at 0, 0.5, 1 (quadratic), two
    /// rotation keys (quadratic) and two scale/shear keys (linear).
    fn split(channel: u32) -> Vec<u8> {
        let mut out = le(&[channel, 0, 1, 2, 2, 1, 3, 2, 2, 0, 1, 2, 0]);
        put_floats(&mut out, &[0.0, 0.5, 1.0, 0.0, 1.0, 0.0, 1.0]);
        put_floats(&mut out, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
        put_floats(&mut out, &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.6, 0.8]);
        put_floats(&mut out, &IDENTITY);
        put_floats(&mut out, &IDENTITY.map(|v| 2.0 * v));
        out
    }

    /// Interleaved track of channel `channel` (from 1) with frames at 0 and 0.02 s.
    fn interleaved(channel: u32) -> Vec<u8> {
        let mut out = le(&[channel, 0, 0]);
        for (k, time) in [0.0, 0.02].into_iter().enumerate() {
            let x = if k == 0 { 1.0 } else { 2.0 };
            put_floats(&mut out, &[time, x, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
            put_floats(&mut out, &IDENTITY);
        }
        out
    }

    /// Two bones (channels 2 and 1, see `skeleton_nodes`) and an animation with `tracks`.
    fn nodes(tracks: &[Vec<u8>]) -> Vec<GrnNode> {
        let mut nodes = skeleton_nodes(&[bone(b"__Root", 0), bone(b"Bip01", 0)]);
        let tracks = tracks
            .iter()
            .map(|t| grn_tree(TAG_TRACK, t, vec![]))
            .collect();
        nodes.extend(grn_tree(
            TAG_ANIMATIONS,
            b"",
            vec![grn_tree(
                TAG_ANIMATION,
                b"",
                vec![
                    grn_tree(0xCA5E_1201, b"", vec![]),
                    grn_tree(TAG_TRACKS, b"", tracks),
                ],
            )],
        ));
        nodes
    }

    fn parse(nodes: Vec<GrnNode>) -> Result<Animation> {
        Grn::parse(&object_grn(nodes)).unwrap().animation()
    }

    #[test]
    fn reads_split_and_interleaved_tracks() {
        let animation = parse(nodes(&[split(2), interleaved(1)])).unwrap();
        let [first, second] = animation.tracks.as_slice() else {
            panic!("{animation:?}");
        };
        assert_eq!((first.channel, first.layout), (1, Layout::Split));
        assert_eq!(first.translation.interpolation, Interpolation::Quadratic);
        assert_eq!(first.translation.times, [0.0, 0.5, 1.0]);
        assert_eq!(first.translation.keys[2], [7.0, 8.0, 9.0]);
        assert_eq!(
            first.rotation.keys,
            [[0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.6, 0.8]]
        );
        assert_eq!(first.scale_shear.interpolation, Interpolation::Linear);
        assert_eq!(first.scale_shear.keys[1][8], 2.0);

        assert_eq!((second.channel, second.layout), (0, Layout::Interleaved));
        for curve_times in [
            &second.translation.times,
            &second.rotation.times,
            &second.scale_shear.times,
        ] {
            assert_eq!(curve_times, &[0.0, 0.02]);
        }
        assert_eq!(second.translation.keys, [[1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
        assert_eq!(second.rotation.interpolation, Interpolation::Linear);
        assert_eq!(animation.duration(), 1.0);

        // The tracks name the bones by their channels.
        let data = object_grn(nodes(&[]));
        let skeleton = Grn::parse(&data).unwrap().skeleton().unwrap();
        assert_eq!(skeleton.bone_of_channel(1), Some(0));
        assert_eq!(skeleton.bone_of_channel(0), Some(1));
        assert_eq!(skeleton.bone_of_channel(2), None);
    }

    #[test]
    fn a_file_without_tracks_has_an_empty_animation() {
        let animation = parse(nodes(&[])).unwrap();
        assert!(animation.tracks.is_empty());
        assert_eq!(animation.duration(), 0.0);
    }

    #[test]
    fn the_channel_must_exist() {
        let err = parse(nodes(&[split(3)])).unwrap_err();
        assert!(
            err.to_string()
                .ends_with("field `track channel` is 3, expected 1..=2"),
            "{err}"
        );
    }

    #[test]
    fn the_split_header_is_checked() {
        for (word, value, message) in [
            (2, 7, "field `track layout` is 7, expected 0 or 1"),
            (4, 3, "field `interpolation mode` is 3, expected 0..=2"),
            (6, 0, "field `key count` is 0, expected at least 1"),
            (10, 2, "field `time selector` is 2, expected 1"),
            (12, 1, "field `track +0x30` is 1, expected 0"),
        ] {
            let mut track = split(1);
            track[4 * word..4 * word + 4].copy_from_slice(&u32::to_le_bytes(value));
            let err = parse(nodes(&[track])).unwrap_err();
            assert!(err.to_string().ends_with(message), "{err}");
        }
        let mut track = split(1);
        track.extend_from_slice(&[0; 4]);
        let err = parse(nodes(&[track])).unwrap_err();
        assert!(
            err.to_string().ends_with(
                "field `split track size` is 224, expected 220, 52 + 16 nt + 20 nq + 40 ns"
            ),
            "{err}"
        );
    }

    #[test]
    fn interleaved_frames_are_68_bytes() {
        let mut track = interleaved(1);
        track.pop();
        let err = parse(nodes(&[track])).unwrap_err();
        assert!(
            err.to_string()
                .ends_with("field `interleaved track size` is 147, expected 12 + 68 per frame"),
            "{err}"
        );
    }

    #[test]
    fn times_rise_strictly_from_zero() {
        // The second translation time equals the first.
        let mut track = split(1);
        track[56..60].copy_from_slice(&0f32.to_le_bytes());
        let mut nodes = nodes(&[track]);
        let err = parse(nodes.clone()).unwrap_err();
        assert_eq!(err.kind.to_string(), "time of key 1 is 0, expected above 0");
        // The first rotation time is not 0.
        grn_node(&mut nodes, TAG_TRACK, 0).data[64..68].copy_from_slice(&1f32.to_le_bytes());
        grn_node(&mut nodes, TAG_TRACK, 0).data[56..60].copy_from_slice(&0.5f32.to_le_bytes());
        let err = parse(nodes).unwrap_err();
        assert_eq!(err.kind.to_string(), "time of key 0 is 1, expected 0");

        let mut track = interleaved(1);
        track[12 + 68..12 + 72].copy_from_slice(&(-1f32).to_le_bytes());
        let err = parse(self::nodes(&[track])).unwrap_err();
        assert_eq!(
            err.kind.to_string(),
            "time of key 1 is -1, expected above 0"
        );
    }
}
