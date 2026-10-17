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

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use bevy::gltf::Gltf;
use bevy::prelude::{
    AnimationGraph, AnimationGraphHandle, AnimationNodeIndex, AnimationPlayer, AssetServer, Assets,
    ChildOf, Commands, Component, Entity, GltfAssetLabel, Handle, Query, Res, ResMut, Resource,
    Transform, Vec3, Without, WorldAssetRoot,
};
use opensacred_data::netscript::{Body, Record, Tree, Value};
use opensacred_data::tables::Item;
use opensacred_data::world::SIDE;

use crate::sectors::Streamer;
use crate::terrain::Heights;
use crate::{hero, iso};

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
            let cell = ops.iter().find_map(|o| match (o.tag, &o.value) {
                (0x04, Value::Name(name)) => named.get(&name.to_ascii_lowercase()).copied(),
                (0x04, Value::I32x3([x, y, _])) => Some((*x, *y)),
                _ => None,
            });
            let heading = ops.iter().find_map(|o| match (o.tag, &o.value) {
                (0x03, Value::U16(v)) => Some(f32::from(*v)),
                _ => None,
            });
            let (Some(creature), Some((x, y))) = (creature, cell) else {
                continue;
            };
            let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
                continue;
            };
            let Some(model) = items
                .get(&creature)
                .and_then(|item| models.get(&item.name.to_ascii_uppercase()))
            else {
                continue;
            };
            placements.push(Placement {
                model: model.clone(),
                cell: (x, y),
                heading: heading.unwrap_or(0.0),
                object: record.kind == CREATE_OBJ,
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
    }
}
