//! Skeleton of a GRN file (docs/02 §4.2): bones with their parents and local transforms in the
//! bind pose. A bone record carries no name; the form names the bones: its list of bone channels
//! gives every bone a transform channel, whose data extension object holds `__ObjectName`. The
//! chain follows grn2glb (`src/core/grn_parser.cpp`, MIT) and is confirmed on every file by
//! `tools/analysis/formats/grn_dump.py`. The file has no inverse bind matrices: the bind pose is
//! the composition of the local transforms from the root.

use super::{Objects, Section, TAG_BONE, one_based};
use crate::error::{ErrorKind, Result, field, to_usize};

const TAG_SKELETONS: u32 = 0xCA5E_0507;
const TAG_SKELETON: u32 = 0xCA5E_0505;
const TAG_BONES: u32 = 0xCA5E_0508;
pub(super) const TAG_FORMS: u32 = 0xCA5E_0C01;
pub(super) const TAG_FORM: u32 = 0xCA5E_0C00;
const TAG_FORM_SKELETONS: u32 = 0xCA5E_0C07;
const TAG_FORM_SKELETON: u32 = 0xCA5E_0C08;
const TAG_BONE_CHANNELS: u32 = 0xCA5E_0C02;
const BONE_SIZE: usize = 68;

/// Bones in file order: a parent precedes its children; bone 0 is the root, `__Root` in every
/// file of the game.
#[derive(Debug, Clone, PartialEq)]
pub struct Skeleton<'a> {
    pub bones: Vec<Bone<'a>>,
}

/// Bone record (68 B): `u32` parent, the local transform relative to the parent.
#[derive(Debug, Clone, PartialEq)]
pub struct Bone<'a> {
    /// `__ObjectName` as stored (Latin-1; tabs in one name). Names repeat within 138 files.
    pub name: &'a [u8],
    /// `None` for the root, bone 0, whose parent field holds its own index 0.
    pub parent: Option<usize>,
    /// Transform channel of the bone, from 0: the channel whose object names the bone and which
    /// the animation tracks refer to.
    pub channel: usize,
    pub translation: [f32; 3],
    /// Unit quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
    /// Scale and shear, 9 values in file order; the identity in most bones. The matrix is
    /// symmetric in every bone of the game, so rows and columns read the same; the local
    /// transform is `T · R · SS`.
    pub scale_shear: [f32; 9],
}

impl<'a> Skeleton<'a> {
    /// The only skeleton of the object section, `0507 → 0505 → 0508 → 0506…`, named through the
    /// only form, `0C01 → 0C00 → 0C07 → 0C08 → 0C02`.
    pub fn parse(section: &Section<'a>, objects: &Objects<'a>) -> Result<Self> {
        let list = section.child(None, TAG_SKELETONS, "GRN skeletons")?;
        let skeleton = section.child(Some(list), TAG_SKELETON, "GRN skeletons")?;
        let list = section.child(Some(skeleton), TAG_BONES, "GRN skeleton")?;
        let bones = section.list(Some(list), TAG_BONE, "GRN bones")?;

        let list = section.child(None, TAG_FORMS, "GRN forms")?;
        let form = section.child(Some(list), TAG_FORM, "GRN forms")?;
        let list = section.child(Some(form), TAG_FORM_SKELETONS, "GRN form")?;
        let binding = section.child(Some(list), TAG_FORM_SKELETON, "GRN form skeletons")?;
        // The form's skeleton, numbered from 1: the file has one.
        one_based(
            &mut binding.sized(4, "form skeleton size")?,
            1,
            "form skeleton",
        )?;
        let mut channels = section
            .child(Some(binding), TAG_BONE_CHANNELS, "GRN form skeleton")?
            .sized(4usize.saturating_mul(bones.len()), "bone channels size")?;

        let mut out = Vec::with_capacity(bones.len());
        for (index, node) in bones.into_iter().enumerate() {
            let channel_at = channels.offset();
            let channel = one_based(&mut channels, objects.channels(), "bone channel")?;
            let name = objects
                .channel(channel)
                .and_then(|object| object.name())
                .ok_or_else(|| {
                    channels.error_at(
                        channel_at,
                        ErrorKind::NoProperty {
                            what: "object of the bone channel",
                            key: "__ObjectName",
                        },
                    )
                })?;

            let mut r = node.sized(BONE_SIZE, "bone size")?;
            let parent_at = r.offset();
            let parent = r.u32()?;
            let parent = match (index, to_usize(parent)) {
                (0, 0) => None,
                (1.., p) if p < index => Some(p),
                _ => {
                    let expected = if index == 0 {
                        "0, the root itself".to_owned()
                    } else {
                        format!("below {index}, a preceding bone")
                    };
                    return Err(
                        r.error_at(parent_at, field("bone parent", parent.into(), expected))
                    );
                }
            };
            out.push(Bone {
                name,
                parent,
                channel,
                translation: r.f32s()?,
                rotation: r.f32s()?,
                scale_shear: r.f32s()?,
            });
        }
        Ok(Self { bones: out })
    }

    /// Index of the first bone named `name`.
    pub fn bone(&self, name: &[u8]) -> Option<usize> {
        self.bones.iter().position(|bone| bone.name == name)
    }

    /// Index of the bone of transform channel `channel` (from 0); the form gives every bone a
    /// channel of its own.
    pub fn bone_of_channel(&self, channel: usize) -> Option<usize> {
        self.bones.iter().position(|bone| bone.channel == channel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grn::Grn;
    use crate::testing::{GrnBone, GrnNode, grn_node, object_grn, skeleton_nodes};

    /// Bone whose 16 transform values are `x`, `x + 0.01`, … in file order.
    fn bone(name: &'static [u8], parent: u32, x: f32) -> GrnBone {
        let transform = std::array::from_fn(|k| x + f32::from(u8::try_from(k).unwrap()) / 100.0);
        GrnBone {
            name,
            parent,
            transform,
        }
    }

    fn sample() -> Vec<GrnNode> {
        skeleton_nodes(&[
            bone(b"__Root", 0, 0.0),
            bone(b"Bip01", 0, 1.0),
            bone(b"Bip01 Pelvis", 1, 2.0),
            bone(b"Bip01 Spine", 2, 3.0),
        ])
    }

    fn error(nodes: Vec<GrnNode>) -> crate::Error {
        Grn::parse(&object_grn(nodes))
            .unwrap()
            .skeleton()
            .unwrap_err()
    }

    #[test]
    fn reads_bones_with_names_and_parents() {
        let data = object_grn(sample());
        let skeleton = Grn::parse(&data).unwrap().skeleton().unwrap();
        let names: Vec<_> = skeleton.bones.iter().map(|b| b.name).collect();
        assert_eq!(
            names,
            [
                b"__Root".as_slice(),
                b"Bip01",
                b"Bip01 Pelvis",
                b"Bip01 Spine"
            ]
        );
        let parents: Vec<_> = skeleton.bones.iter().map(|b| b.parent).collect();
        assert_eq!(parents, [None, Some(0), Some(1), Some(2)]);
        let pelvis = &skeleton.bones[2];
        assert_eq!(pelvis.translation, [2.0, 2.01, 2.02]);
        assert_eq!(pelvis.rotation, [2.03, 2.04, 2.05, 2.06]);
        assert_eq!(
            pelvis.scale_shear,
            [2.07, 2.08, 2.09, 2.1, 2.11, 2.12, 2.13, 2.14, 2.15]
        );
        assert_eq!(skeleton.bone(b"Bip01 Spine"), Some(3));
        assert_eq!(skeleton.bone(b"Bip01 Head"), None);
    }

    #[test]
    fn the_root_is_its_own_parent() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_BONE, 0).data[0] = 1;
        let err = error(nodes);
        assert!(
            err.to_string()
                .ends_with("field `bone parent` is 1, expected 0, the root itself"),
            "{err}"
        );
    }

    #[test]
    fn a_parent_precedes_its_bone() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_BONE, 2).data[0] = 2;
        let err = error(nodes);
        assert!(
            err.to_string()
                .ends_with("field `bone parent` is 2, expected below 2, a preceding bone"),
            "{err}"
        );
    }

    #[test]
    fn bone_records_are_68_bytes() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_BONE, 1).data.pop();
        let err = error(nodes);
        assert!(
            err.to_string()
                .ends_with("field `bone size` is 67, expected 68"),
            "{err}"
        );
    }

    #[test]
    fn the_bone_list_holds_only_bones() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_BONE, 3).tag = TAG_SKELETONS;
        let err = error(nodes);
        assert_eq!(
            err.kind.to_string(),
            "GRN bones: tag 0xca5e0507, expected 0xca5e0506"
        );
    }

    #[test]
    fn the_skeleton_is_required() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_SKELETONS, 0).tag = 0xCA5E_0A01;
        let err = error(nodes);
        assert_eq!(
            err.kind.to_string(),
            "GRN skeletons: 0 with tag 0xca5e0507, expected one"
        );
    }

    #[test]
    fn the_form_binds_the_only_skeleton() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_FORM_SKELETON, 0).data[0] = 2;
        let err = error(nodes);
        assert!(
            err.to_string()
                .ends_with("field `form skeleton` is 2, expected 1..=1"),
            "{err}"
        );
    }

    #[test]
    fn every_bone_has_a_channel() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_BONE_CHANNELS, 0).data.truncate(12);
        let err = error(nodes);
        assert!(
            err.to_string()
                .ends_with("field `bone channels size` is 12, expected 16"),
            "{err}"
        );

        let mut nodes = sample();
        grn_node(&mut nodes, TAG_BONE_CHANNELS, 0).data[4] = 5;
        let err = error(nodes);
        assert!(
            err.to_string()
                .ends_with("field `bone channel` is 5, expected 1..=4"),
            "{err}"
        );
    }

    #[test]
    fn a_bone_object_must_carry_a_name() {
        // The first object, bone 0, gets the empty key instead of `__ObjectName`.
        let mut nodes = sample();
        grn_node(&mut nodes, 0xCA5E_0F01, 0).data = 0u32.to_le_bytes().to_vec();
        let err = error(nodes);
        assert_eq!(
            err.kind.to_string(),
            "object of the bone channel: no single value of property `__ObjectName`"
        );
    }
}
