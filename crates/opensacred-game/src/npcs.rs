//! The NPCs and objects of the start script: the `CreateNPC` and `CreateObj` records of the hero's
//! tree's `StartCode.bin` (`netscript/<tree>.nsc`), each placed when its sector is in; an NPC plays
//! an idle clip of its model, an object (a barrel, a chest, a book stand, a signpost) stands.
//!
//! What the operands mean is read off the data (**hypothesis**, docs/02 §7.3): the first `0x02`
//! is the creature, which is also the `items.pak` slot of its model (`SOLDIER.GRN`, `HORSE.GRN`),
//! the further `0x02` its equipment (swords, shields: not attached yet); `0x04` its position, the
//! name of a `DefPos` record (whose two `0x0B` give the global cell) or a global cell `(x, y, 0)`;
//! `0x03` a heading in degrees, which the client turns into the direction `(cos, sin)` in the
//! grid's axes (`0x44DDC0`, docs/02 §7.3). A `CreateObj` record has the same `0x02`, `0x04` and
//! `0x03`, and names its trigger and its contents in `0x29` and `0x41` (**hypothesis**, by the
//! same reading).
//!
//! A position's third number is the layer the object lies in (**hypothesis**, docs/re/drawing.md):
//! the client draws a building's cells with the world's objects of the building's current layer,
//! layer 0 while the hero is outside. The game has no buildings' layers yet (#3), so the objects
//! of the upper layers, the furniture inside the houses, stay hidden: drawn, they stood over the
//! houses' walls.
//!
//! The model takes the texture its `items.pak` record names at `+0x08`, a `texture.pak` record,
//! in place of the GRN's own (docs/02 §6.1): the chests, vases and coffins of `chest.grn`,
//! `vase_01.grn` and `coffin_01.grn` have none of their own, and the soldiers, horses, wolves
//! and citizens wear their record's variant of their model's skin.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use bevy::gltf::Gltf;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::{
    AnimationGraph, AnimationGraphHandle, AnimationNodeIndex, AnimationPlayer, AssetId,
    AssetServer, Assets, ChildOf, Children, Commands, Component, Entity, GltfAssetLabel, Handle,
    Image, MeshMaterial3d, On, Query, Res, ResMut, Resource, StandardMaterial, Transform, Vec3,
    Visibility, Without, WorldAssetRoot,
};
use bevy::world_serialization::WorldInstanceReady;
use opensacred_data::netscript::{Body, Record, Tree, Value};
use opensacred_data::tables::Item;
use opensacred_data::world::SIDE;

use crate::sectors::Streamer;
use crate::terrain::Heights;
use crate::{buildings, hero, iso};

/// Record types of the start script (`opensacred_formats::netscript::KEYWORDS`).
const CREATE_NPC: u16 = 1;
const CREATE_OBJ: u16 = 8;
const DEF_POS: u16 = 23;
const START_POSITION: u16 = 45;

/// An NPC or an object to place.
struct Placement {
    /// The model, relative to the asset directory.
    model: String,
    /// The global cell.
    cell: (u32, u32),
    /// Degrees.
    heading: f32,
    /// A `CreateObj`'s object rather than an NPC.
    object: bool,
    /// The texture its item names (`+0x08`), relative to the asset directory.
    texture: Option<String>,
    /// The position's third number, the layer.
    layer: i32,
}

#[derive(Resource)]
pub struct Npcs {
    placements: Vec<Placement>,
    by_sector: HashMap<(u32, u32), Vec<usize>>,
    spawned: HashSet<(u32, u32)>,
}

/// An object of the start script.
#[derive(Component)]
pub struct Object;

/// A placed NPC or object of a layer above 0: inside a building, shown with its layer
/// (`buildings::Furnishing`), or never outside one.
#[derive(Component)]
pub struct UpperLayer;

/// The texture a placed model takes in place of its own: its item's (`items.pak +0x08`).
#[derive(Component)]
pub struct Retexture(Handle<Image>);

/// The glTF materials given an item's texture, by the material and the texture: shared by the
/// models of one item.
#[derive(Resource, Default)]
pub struct Retextured(
    HashMap<(AssetId<StandardMaterial>, AssetId<Image>), Handle<StandardMaterial>>,
);

/// An NPC's model root, until its idle clip plays.
#[derive(Component)]
pub struct Npc {
    gltf: Handle<Gltf>,
    graph: Option<(Handle<AnimationGraph>, AnimationNodeIndex)>,
}

fn value_i32(value: &Value) -> Option<i32> {
    match value {
        Value::I32(v) => Some(*v),
        _ => None,
    }
}

/// The direction of a heading in degrees on the ground, as the client turns it (`0x44DDC0`):
/// 0° along the grid's x (the world's X), 90° along its y (the world's Z).
fn heading(degrees: f32) -> Vec3 {
    let (sin, cos) = degrees.to_radians().sin_cos();
    Vec3::new(cos, 0.0, sin)
}

/// The hero's start: the global cell of the `StartPosition` record of `tree`'s start script
/// (`StartPosition 0B:x 0B:y`), if it has one.
pub fn start_position(assets: &Path, tree: &str) -> Result<Option<(u32, u32)>> {
    let path = assets.join("netscript").join(format!("{tree}.nsc"));
    let data = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let tree = Tree::decode(&data).with_context(|| format!("decoding {}", path.display()))?;
    Ok(tree
        .start
        .iter()
        .filter(|r| r.kind == START_POSITION)
        .find_map(|r| {
            let Body::Operands(ops) = &r.body else {
                return None;
            };
            let x = u32::try_from(value_i32(&ops.first()?.value)?).ok()?;
            let y = u32::try_from(value_i32(&ops.get(1)?.value)?).ok()?;
            Some((x, y))
        }))
}

/// The named positions of the start script: `DefPos` name, x, y.
fn positions(records: &[Record]) -> HashMap<String, (i32, i32)> {
    records
        .iter()
        .filter(|r| r.kind == DEF_POS)
        .filter_map(|r| {
            let Body::Operands(ops) = &r.body else {
                return None;
            };
            let Value::Str(name) = &ops.first()?.value else {
                return None;
            };
            let x = value_i32(&ops.get(1)?.value)?;
            let y = value_i32(&ops.get(2)?.value)?;
            Some((name.to_ascii_lowercase(), (x, y)))
        })
        .collect()
}

/// A record's position (`0x04`): the global cell and the layer, of a `DefPos` name (layer 0) or
/// given as `(x, y, layer)`.
fn position(
    ops: &[opensacred_data::netscript::Operand],
    named: &HashMap<String, (i32, i32)>,
) -> Option<(i32, i32, i32)> {
    ops.iter().find_map(|o| match (o.tag, &o.value) {
        (0x04, Value::Name(name)) => named
            .get(&name.to_ascii_lowercase())
            .map(|&(x, y)| (x, y, 0)),
        (0x04, Value::I32x3([x, y, layer])) => Some((*x, *y, *layer)),
        _ => None,
    })
}

/// The PNG of every `texture.pak` record (`textures/texture/NNNNN-name.png`), by its number,
/// relative to the asset directory.
pub fn texture_files(assets: &Path) -> Result<HashMap<u32, String>> {
    let path = assets.join("textures").join("texture");
    let mut files = HashMap::new();
    let Ok(entries) = fs::read_dir(&path) else {
        return Ok(files);
    };
    for entry in entries {
        let name = entry
            .with_context(|| format!("reading {}", path.display()))?
            .file_name()
            .to_string_lossy()
            .into_owned();
        if let Some(number) = name
            .split_once('-')
            .and_then(|(number, _)| number.parse::<u32>().ok())
            && name.ends_with(".png")
        {
            files.insert(number, format!("textures/texture/{name}"));
        }
    }
    Ok(files)
}

/// The `.glb` of every model, by its upper-case `.GRN` name.
pub fn model_files(assets: &Path) -> Result<HashMap<String, String>> {
    let mut files = HashMap::new();
    for dir in ["models", "models03"] {
        let path = assets.join("models").join(dir);
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries {
            let name = entry
                .with_context(|| format!("reading {}", path.display()))?
                .file_name()
                .to_string_lossy()
                .into_owned();
            let Some(stem) = name.strip_suffix(".glb") else {
                continue;
            };
            let Some((_, grn)) = stem.split_once('-') else {
                continue;
            };
            files
                .entry(format!("{}.GRN", grn.to_ascii_uppercase()))
                .or_insert(format!("models/{dir}/{name}"));
        }
    }
    Ok(files)
}

impl Npcs {
    /// Reads the start script of `tree` and the item table.
    pub fn read(assets: &Path, tree: &str) -> Result<Self> {
        let path = assets.join("netscript").join(format!("{tree}.nsc"));
        let data = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let tree = Tree::decode(&data).with_context(|| format!("decoding {}", path.display()))?;
        let path = assets.join("tables").join(opensacred_data::tables::ITEMS);
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let items: Vec<Item> =
            ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        let items: HashMap<u32, &Item> = items.iter().map(|i| (i.slot, i)).collect();
        let models = model_files(assets)?;
        let textures = texture_files(assets)?;
        let named = positions(&tree.start);
        let mut placements = Vec::new();
        for record in tree
            .start
            .iter()
            .filter(|r| r.kind == CREATE_NPC || r.kind == CREATE_OBJ)
        {
            let Body::Operands(ops) = &record.body else {
                continue;
            };
            let creature = ops.iter().find_map(|o| match (o.tag, &o.value) {
                (0x02, Value::I32(v)) => u32::try_from(*v).ok(),
                _ => None,
            });
            let cell = position(ops, &named);
            let heading = ops.iter().find_map(|o| match (o.tag, &o.value) {
                (0x03, Value::U16(v)) => Some(f32::from(*v)),
                _ => None,
            });
            let (Some(creature), Some((x, y, layer))) = (creature, cell) else {
                continue;
            };
            let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
                continue;
            };
            let Some(item) = items.get(&creature) else {
                continue;
            };
            let Some(model) = models.get(&item.name.to_ascii_uppercase()) else {
                continue;
            };
            let texture = match item.words_08[0] {
                0 => None,
                number => textures.get(&number).cloned(),
            };
            placements.push(Placement {
                model: model.clone(),
                cell: (x, y),
                heading: heading.unwrap_or(0.0),
                object: record.kind == CREATE_OBJ,
                texture,
                layer,
            });
        }
        let side = u32::try_from(SIDE)?;
        let mut by_sector: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (index, placement) in placements.iter().enumerate() {
            let (x, y) = placement.cell;
            by_sector
                .entry((x / side, y / side))
                .or_default()
                .push(index);
        }
        Ok(Self {
            placements,
            by_sector,
            spawned: HashSet::new(),
        })
    }

    /// The NPCs and the objects to place.
    pub fn counts(&self) -> (usize, usize) {
        let objects = self.placements.iter().filter(|p| p.object).count();
        (self.placements.len() - objects, objects)
    }
}

/// Places the NPCs of every sector that came in, as entities of that sector.
pub fn place(
    mut commands: Commands,
    mut npcs: ResMut<Npcs>,
    mut streamer: ResMut<Streamer>,
    heights: Res<Heights>,
    assets: Res<AssetServer>,
) {
    let npcs = &mut *npcs;
    let loaded: Vec<(u32, u32)> = streamer.sectors().collect();
    for &at in &loaded {
        if npcs.spawned.contains(&at) {
            continue;
        }
        npcs.spawned.insert(at);
        for &index in npcs
            .by_sector
            .get(&at)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            let placement = &npcs.placements[index];
            let (x, y) = placement.cell;
            let mut position = iso::ground(x as f32 + 0.5, y as f32 + 0.5);
            position.y = heights.at(position.x, position.z).unwrap_or(0.0);
            let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(placement.model.clone()));
            let mut entity = commands.spawn((
                WorldAssetRoot(scene),
                Transform::from_translation(position)
                    .with_rotation(hero::facing(heading(placement.heading)))
                    .with_scale(Vec3::splat(iso::MODEL_SCALE)),
            ));
            if placement.layer != 0 {
                // In a building's layer: shown with it; else never.
                match heights.building(position.x, position.z) {
                    Some((building, _)) => entity.insert((
                        buildings::Furnishing {
                            building,
                            layer: placement.layer,
                        },
                        UpperLayer,
                        Visibility::Hidden,
                    )),
                    None => entity.insert((UpperLayer, Visibility::Hidden)),
                };
            }
            if let Some(texture) = &placement.texture {
                // Repeated, as a glTF texture without a sampler is: the models' texture
                // coordinates run past the image (the lid of `chest.grn` from -0.99 to -0.36).
                let texture = assets
                    .load_builder()
                    .with_settings(|settings: &mut ImageLoaderSettings| {
                        let mut sampler = ImageSamplerDescriptor::linear();
                        sampler.set_address_mode(ImageAddressMode::Repeat);
                        settings.sampler = ImageSampler::Descriptor(sampler);
                    })
                    .load(texture.clone());
                entity.insert(Retexture(texture));
            }
            if placement.object {
                entity.insert(Object);
            } else {
                entity.insert(Npc {
                    gltf: assets.load(placement.model.clone()),
                    graph: None,
                });
            }
            let entity = entity.id();
            streamer.adopt(at, entity);
        }
    }
    // Sectors that went out place their NPCs again when they come back.
    npcs.spawned.retain(|at| loaded.contains(at));
}

/// Gives the meshes of a placed model that has just spawned its item's texture, each glTF
/// material copied once per texture with that texture as its base colour.
pub fn retexture(
    ready: On<WorldInstanceReady>,
    models: Query<&Retexture>,
    children: Query<&Children>,
    mut meshes: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut retextured: ResMut<Retextured>,
) {
    let Ok(Retexture(texture)) = models.get(ready.entity) else {
        return;
    };
    for entity in children.iter_descendants(ready.entity) {
        let Ok(mut material) = meshes.get_mut(entity) else {
            continue;
        };
        let key = (material.0.id(), texture.id());
        let handle = match retextured.0.get(&key) {
            Some(handle) => handle.clone(),
            None => {
                let Some(original) = materials.get(&material.0) else {
                    continue;
                };
                let mut copy = original.clone();
                copy.base_color_texture = Some(texture.clone());
                let handle = materials.add(copy);
                retextured.0.insert(key, handle.clone());
                handle
            }
        };
        material.0 = handle;
    }
}

/// Gives each NPC's animation player an idle clip of its model: the first by name that holds
/// `IDLE` but not `FUN`, else one with `IDLE`, else the first clip.
pub fn animate(
    mut commands: Commands,
    mut npcs: Query<&mut Npc>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut players: Query<(Entity, &mut AnimationPlayer), Without<AnimationGraphHandle>>,
    parents: Query<&ChildOf>,
) {
    for (entity, mut player) in &mut players {
        // The NPC this player belongs to: the nearest ancestor with `Npc`.
        let mut ancestor = entity;
        let npc = loop {
            if let Ok(npc) = npcs.get_mut(ancestor) {
                break Some(npc);
            }
            match parents.get(ancestor) {
                Ok(parent) => ancestor = parent.parent(),
                Err(_) => break None,
            }
        };
        let Some(mut npc) = npc else {
            continue;
        };
        if npc.graph.is_none() {
            let Some(gltf) = gltfs.get(&npc.gltf) else {
                continue;
            };
            // The plain idle before the fidgets (`*_FUN_IDLE_*`, a horse rearing), by name.
            let mut names: Vec<&str> = gltf.named_animations.keys().map(|n| &**n).collect();
            names.sort_unstable();
            let idle = names
                .iter()
                .find(|n| n.contains("IDLE") && !n.contains("FUN"))
                .or_else(|| names.iter().find(|n| n.contains("IDLE")))
                .and_then(|n| gltf.named_animations.get(*n));
            let clip = idle.cloned().or_else(|| gltf.animations.first().cloned());
            let Some(clip) = clip else {
                continue;
            };
            let (graph, node) = AnimationGraph::from_clip(clip);
            npc.graph = Some((graphs.add(graph), node));
        }
        if let Some((graph, node)) = npc.graph.clone() {
            player.play(node).repeat();
            commands.entity(entity).insert(AnimationGraphHandle(graph));
        }
    }
}

#[cfg(test)]
mod tests {
    use opensacred_data::netscript::Operand;

    use super::*;

    #[test]
    fn textures_are_found_by_their_record_number() {
        let assets =
            std::env::temp_dir().join(format!("opensacred-game-textures-{}", std::process::id()));
        let dir = assets.join("textures").join("texture");
        fs::create_dir_all(&dir).unwrap();
        for name in ["07442-chest02.png", "00001-cab.png", "notes.txt", "x-y.png"] {
            fs::write(dir.join(name), b"").unwrap();
        }
        let files = texture_files(&assets).unwrap();
        fs::remove_dir_all(&assets).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[&7442], "textures/texture/07442-chest02.png");
        assert_eq!(files[&1], "textures/texture/00001-cab.png");
    }

    #[test]
    fn a_heading_turns_from_the_grid_x_toward_its_y() {
        let close = |a: Vec3, b: Vec3| (a - b).length() < 1e-6;
        assert!(close(heading(0.0), Vec3::X));
        assert!(close(heading(90.0), Vec3::Z));
        assert!(close(heading(180.0), -Vec3::X));
    }

    #[test]
    fn def_pos_records_name_global_cells() {
        let op = |tag, value| Operand { tag, value };
        let records = vec![
            Record {
                kind: DEF_POS,
                body: Body::Operands(vec![
                    op(0x01, Value::Str("Hauptmann3".into())),
                    op(0x0B, Value::I32(3349)),
                    op(0x0B, Value::I32(2534)),
                ]),
            },
            Record {
                kind: CREATE_NPC,
                body: Body::Operands(vec![op(0x02, Value::I32(297))]),
            },
        ];
        let named = positions(&records);
        assert_eq!(named.len(), 1);
        assert_eq!(named["hauptmann3"], (3349, 2534));
        // A name is on layer 0; a cell carries its layer.
        let at = |value| position(&[op(0x02, Value::I32(5207)), op(0x04, value)], &named);
        assert_eq!(at(Value::Name("HAUPTMANN3".into())), Some((3349, 2534, 0)));
        assert_eq!(at(Value::I32x3([3259, 2569, 1])), Some((3259, 2569, 1)));
        assert_eq!(at(Value::Name("elsewhere".into())), None);
    }
}
