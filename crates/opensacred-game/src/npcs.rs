//! The NPCs and objects of the start script: the `CreateNPC` and `CreateObj` records of the hero's
//! tree's `StartCode.bin` (`netscript/<tree>.nsc`), of the `OnEnter` functions of the quests
//! that the start script enters (the novice in the chapel, quest 1's), and of the sectors' `Init`
//! and `Enter` functions (the villagers, the animals and the placed monsters, docs/re/spawning.md),
//! each placed when its sector is in; an NPC plays the clip of its model that the client plays standing, by the class of what
//! it holds (`motions`), an object (a barrel, a chest, a book stand, a signpost) stands, and a
//! chest, a barrel or a book stand takes its cell, which the creatures go round
//! (`terrain::Heights::occupy`, docs/re/movement.md, "The objects in the way"). An NPC
//! that a quest sends somewhere at the start (`NPC_Goto`) walks there from where it was made,
//! along the hero's way, with its walk clip at its creature's walk speed, or its run's where its
//! model has no walk, then stands (docs/decisions/2026-10-19-quest-npcs.md).
//!
//! What the operands mean is read off the data (**hypothesis**, docs/02 §7.3): the first `0x02`
//! is the creature, which is also the `items.pak` slot of its model (`SOLDIER.GRN`, `HORSE.GRN`),
//! the further `0x02` its equipment (swords, bows, shields); `0x04` its position, the
//! name of a `DefPos` record (whose two `0x0B` give the global cell) or a global cell `(x, y, 0)`;
//! `0x03` a heading in degrees, an angle on the screen that the client takes into the grid's axes
//! (`0x44DDC0`, docs/02 §7.3, `heading`). A `CreateObj` record has the same `0x02`, `0x04` and
//! `0x03`, and names its trigger and its contents in `0x29` and `0x41` (**hypothesis**, by the
//! same reading); an object without a heading above 0 faces its item's own angle
//! (`Item::angle`).
//!
//! A position's third number is the layer the object lies in (**hypothesis**, docs/re/drawing.md):
//! the client draws every cell with the world's objects of layer 0, and a building's cells with
//! those of the building's current layer too, so an object or NPC on a building's floor shows
//! with its building's state (`buildings::Layered`). A walking NPC changes layers as the hero
//! does, onto a building's inside through its entrance and out at an exit (`0x5FCCB0`).
//!
//! An NPC holds its weapons and its shield as the client equips them (docs/re/equipment.md): each
//! item's model is attached by its socket bone, `Bone_weapon_02` in the left hand and
//! `Bone_weapon_01` in the right, to the NPC's hand, or its forearm for a shield.
//!
//! The model takes the texture its `items.pak` record names at `+0x08`, a `texture.pak` record,
//! in place of the GRN's own (docs/02 §6.1): the chests, vases and coffins of `chest.grn`,
//! `vase_01.grn` and `coffin_01.grn` have none of their own, and the soldiers, horses, wolves
//! and citizens wear their record's variant of their model's skin.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use bevy::gltf::{Gltf, GltfNode};
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::{
    AnimationGraph, AnimationGraphHandle, AnimationNodeIndex, AnimationPlayer,
    AnimationTransitions, App, AssetId, AssetServer, Assets, ChildOf, Children, Commands,
    Component, DetectChangesMut, Entity, GltfAssetLabel, Handle, Has, Image, IntoScheduleConfigs,
    MeshMaterial3d, Name, On, Plugin, Query, Res, ResMut, Resource, StandardMaterial, Time,
    Transform, Update, Vec3, Without, WorldAssetRoot,
};
use bevy::world_serialization::WorldInstanceReady;
use opensacred_data::netscript::{Body, Function, Record, Tree, Value};
use opensacred_data::tables::{Creature, Item, Weapon};
use opensacred_data::world::SIDE;

use crate::frame::Step;
use crate::sectors::Streamer;
use crate::terrain::Heights;
use crate::terrain::Level;
use crate::{buildings, crowd, hero, iso, models, motions, paths, shadows, spawning};

/// Record types of the start script (`opensacred_formats::netscript::KEYWORDS`).
const CREATE_NPC: u16 = 1;
const CREATE_OBJ: u16 = 8;
const TRIGGER_QUEST: u16 = 20;
const SET_UP_QUEST: u16 = 21;
const DEF_POS: u16 = 23;
const CREATE_TRIGGER: u16 = 48;
const TRIGGER_PATCH: u16 = 50;
const FILL_SECTOR: u16 = 51;
const SPAWN_VALUES: u16 = 100;
/// The hero's level, for the fills' counts: the game has no levels yet.
const HERO_LEVEL: u32 = 1;
/// The tags of `CreateTrigger` that set and clear bit 0 of the trigger's state, its open bit
/// (`0x496520`).
const OPEN: u8 = 0x26;
const CLOSED: u8 = 0x27;
const START_POSITION: u16 = 45;
const NPC_GOTO: u16 = 72;

/// An NPC or an object to place.
#[derive(Clone)]
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
    /// The items an NPC holds.
    equipment: Vec<Held>,
    /// Whether it casts a shadow (`shadows::Casts`).
    casts: bool,
    /// Whether it stops the NPCs' steps into its cell, an object of category 4 (`0x4F7E10`
    /// refuses its cell to a creature of id above 16, not to the hero; `crowd`).
    blocks: bool,
    /// An NPC's creature, its `Creature.pak` record's flags and the flags word its record's tags
    /// make (`crowd::flags_word`): how it stands in the others' way.
    creature: (u32, u8, u32),
    /// Where an NPC walks from the start (`NPC_Goto`), the global cell, and its speed in world
    /// units a second, before the game's speed: its walk speed, or its run speed where its walk
    /// is a run.
    walk: Option<((u32, u32), f32)>,
    /// The clips an NPC plays.
    clips: ClipNames,
}

/// The clips an NPC plays, by name (`motions`): standing, walking where a script sends it, and
/// its fight stance where its model has one (`motions::Slots::stance`).
#[derive(Clone, Debug, Default)]
struct ClipNames {
    stand: Option<String>,
    walk: Option<String>,
    stance: Option<String>,
}

/// The item categories of `items.pak +0x2E` that go in a hand (docs/02 §6.1).
const WEAPON: u8 = 5;
const SHIELD: u8 = 13;
/// The category of the chests, the barrels, the vases, the coffins, the cabinets, the book stands
/// and the shrines, which the client makes `cChest3D`s, whose draw draws their shadows
/// (`0x451300`, `0x5C97B0`), and which take their cells: a creature does not step into a cell
/// that holds one (`0x4F7E10`); the signposts are of 15, `cSpecial3D`, which casts none and
/// takes no cell.
const CHEST: u8 = 4;
/// The flag of a creature that casts no shadow (`Creature.pak +0x06`, `0x426D30`).
const NO_SHADOW: u8 = 0x10;
/// The weapon kinds of `Weapon.pak +0x1E` held in the left hand: bows and crossbows.
const BOW: u8 = 7;
const CROSSBOW: u8 = 12;

/// A hand of an NPC's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hand {
    Left,
    Right,
}

/// An item an NPC holds.
#[derive(Clone)]
struct Held {
    /// The model, relative to the asset directory.
    model: String,
    /// The texture its item names (`+0x08`).
    texture: Option<String>,
    hand: Hand,
    /// A shield, held on the forearm rather than in the hand.
    shield: bool,
}

/// The hand each of an NPC's items goes in, as the client equips them in order (`0x54ACC0`,
/// `0x54B130`), from each item's category and, for a weapon, its kind: a shield, a bow or a
/// crossbow goes in the left hand, any other weapon in the right, or in the left when the right
/// is taken. An item whose hand is taken, or that goes in no hand, is not held.
fn hands(items: &[(u8, Option<u8>)]) -> Vec<Option<Hand>> {
    let (mut left, mut right) = (false, false);
    items
        .iter()
        .map(|&(category, kind)| {
            let wanted = match (category, kind) {
                (SHIELD, _) | (WEAPON, Some(BOW | CROSSBOW)) => Hand::Left,
                (WEAPON, _) => Hand::Right,
                _ => return None,
            };
            if wanted == Hand::Right && !right {
                right = true;
                Some(Hand::Right)
            } else if !left && (wanted == Hand::Left || category == WEAPON) {
                left = true;
                Some(Hand::Left)
            } else {
                None
            }
        })
        .collect()
}

/// The class of what an NPC holds (`motions::class`): its items' categories and kinds in the
/// hands they go in (`hands`), a weapon without a kind of class 0.
fn held_class(items: &[(u8, Option<u8>)]) -> u8 {
    let (mut left, mut right) = (None, None);
    for (&(category, kind), hand) in items.iter().zip(hands(items)) {
        match hand {
            Some(Hand::Left) => left = Some((category == SHIELD, kind.unwrap_or(0))),
            Some(Hand::Right) => right = Some(kind.unwrap_or(0)),
            None => {}
        }
    }
    motions::class(left, right)
}

#[derive(Resource)]
pub struct Npcs {
    placements: Vec<Placement>,
    by_sector: HashMap<(u32, u32), Vec<usize>>,
    /// The cells of the closed triggers ([`closed_cells`]), by sector.
    closed: HashMap<(u32, u32), Vec<Spot>>,
    /// The sectors' fills ([`fills`]), and what each type they make is placed as.
    fills: HashMap<(u32, u32), (spawning::SpawnValues, Vec<spawning::Fill>)>,
    templates: HashMap<u32, Placement>,
    /// Each creature type's `Creature.pak` class, which picks a fill's factor.
    classes: HashMap<u32, u8>,
    factors: spawning::Factors,
    spawned: HashSet<(u32, u32)>,
}

/// A global cell and a floor (or a layer), as the scripts give a position: `(x, y, floor)`.
type Spot = (i32, i32, i32);

/// An object of the start script.
#[derive(Component)]
pub struct Object;

/// The texture a placed model takes in place of its own: its item's (`items.pak +0x08`).
#[derive(Component)]
pub struct Retexture(Handle<Image>);

/// The glTF materials given an item's texture, by the material and the texture: shared by the
/// models of one item.
#[derive(Resource, Default)]
pub struct Retextured(
    HashMap<(AssetId<StandardMaterial>, AssetId<Image>), Handle<StandardMaterial>>,
);

/// An NPC's model root.
#[derive(Component)]
pub struct Npc {
    gltf: Handle<Gltf>,
    /// The clips it plays, by name.
    names: ClipNames,
    graph: Option<Clips>,
    /// The entity of its model's animation player, once it has its clips.
    player: Option<Entity>,
}

/// An NPC's animation graph: its idle clip, and its walk clip if it has one.
#[derive(Clone)]
struct Clips {
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    walk: Option<AnimationNodeIndex>,
    stance: Option<AnimationNodeIndex>,
}

/// An NPC walking where a script sent it (`NPC_Goto`, `0x49E210`, which orders the creature to
/// walk there): the point, the level it walks on, and its walk speed in world units a second,
/// before the game's speed.
#[derive(Component)]
pub struct Walk {
    target: Vec3,
    level: Level,
    speed: f32,
    /// Whether it stands on a stair it has climbed already or arrived on.
    climbed: bool,
}

fn value_i32(value: &Value) -> Option<i32> {
    match value {
        Value::I32(v) => Some(*v),
        _ => None,
    }
}

/// The direction on the ground of a heading in degrees, as the client turns it (`0x44DDC0`): the
/// direction `(cos, sin)` on the screen, 0° to the right and 90° down, which `0x622660` takes
/// back through the projection of the cells, a cell's x running `(48, 24)` pixels and its y
/// `(−48, 24)`, into the grid's axes (docs/02 §7.3). The model then shows at that angle on the
/// screen.
fn heading(degrees: f32) -> Vec3 {
    let (sin, cos) = degrees.to_radians().sin_cos();
    Vec3::new(cos + 2.0 * sin, 0.0, 2.0 * sin - cos).normalize()
}

/// The heading a placement faces: an object's when above 0, else its item's own angle, which
/// it takes as it is made (`CreateObj`'s handler `0x485C10` turns it only for a heading above
/// 0); an NPC's, or 0 without one, where the client takes a random one (`0x482510`).
fn facing_degrees(object: bool, heading: Option<f32>, angle: f32) -> f32 {
    match (object, heading) {
        (true, Some(heading)) if heading > 0.0 => heading,
        (true, _) => angle,
        (false, heading) => heading.unwrap_or(0.0),
    }
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

/// The function of `tree` named `name`, whatever the case.
fn function<'a>(tree: &'a Tree, name: &str) -> Option<&'a Function> {
    tree.functions
        .iter()
        .find(|f| f.name.eq_ignore_ascii_case(name))
}

/// The quest a `SetUpQuest` or a `TriggerQuest` record names, its first operand.
fn quest_of(record: &Record) -> Option<i32> {
    let Body::Operands(ops) = &record.body else {
        return None;
    };
    value_i32(&ops.first()?.value)
}

/// The quests that the start script enters as the world starts, in order (docs/re/netscript.md,
/// "Quests"): a new game runs the start script through, and its `TriggerQuest` of a quest that a
/// `SetUpQuest` before it has set up runs the quest's trigger function, `QIS_Trigger<n>`, then
/// enters the quest if it ran through (`0x48F030`, `0x46BA90`); an empty trigger runs through,
/// a missing one does not, and one with records is taken as not run through, the game having no
/// conditions yet.
fn entered_quests(tree: &Tree) -> Vec<i32> {
    let mut set_up = HashSet::new();
    let mut entered = Vec::new();
    for record in &tree.start {
        let Some(quest) = quest_of(record) else {
            continue;
        };
        match record.kind {
            SET_UP_QUEST => {
                set_up.insert(quest);
            }
            TRIGGER_QUEST
                if set_up.contains(&quest)
                    && !entered.contains(&quest)
                    && function(tree, &format!("QIS_Trigger{quest}"))
                        .is_some_and(|f| f.records.is_empty()) =>
            {
                entered.push(quest);
            }
            _ => {}
        }
    }
    entered
}

/// The records that place the world's NPCs and objects: the start script's, then those of the
/// `OnEnter` function of each quest it enters, which the client runs as it enters the quest
/// (`QIS_OnEnter<n>`, `0x46C160`), then the sectors' `Init` and `Enter` functions', which the
/// engine runs as their sectors come near (docs/re/spawning.md); each record's NPC or object is
/// placed as the sector of its cell comes in.
fn start_records(tree: &Tree) -> Vec<&Record> {
    let mut records: Vec<&Record> = tree.start.iter().collect();
    for quest in entered_quests(tree) {
        if let Some(on_enter) = function(tree, &format!("QIS_OnEnter{quest}")) {
            records.extend(&on_enter.records);
        }
    }
    for sector in tree
        .functions
        .iter()
        .filter(|f| sector_function(&f.name).is_some())
    {
        records.extend(&sector.records);
    }
    records
}

/// The `FillSector` records of the sectors' `Enter` functions, by sector, each sector's with the
/// `SpawnValues` of its function (docs/re/spawning.md, "`FillSector`").
fn fills(tree: &Tree) -> HashMap<(u32, u32), (spawning::SpawnValues, Vec<spawning::Fill>)> {
    let mut out = HashMap::new();
    for function in tree.functions.iter().filter(|f| f.name.ends_with("Enter")) {
        let Some(sector) = sector_function(&function.name) else {
            continue;
        };
        let mut values = spawning::SpawnValues::default();
        let mut fills = Vec::new();
        for record in &function.records {
            let Body::Operands(ops) = &record.body else {
                continue;
            };
            match record.kind {
                SPAWN_VALUES => {
                    let numbers: Vec<u32> = ops
                        .iter()
                        .filter_map(|o| match (o.tag, &o.value) {
                            (0x0B, Value::I32(v)) => u32::try_from(*v).ok(),
                            _ => None,
                        })
                        .collect();
                    if let [density, lowest, highest, ..] = numbers[..] {
                        values = spawning::SpawnValues {
                            density,
                            lowest,
                            highest,
                        };
                    }
                }
                FILL_SECTOR => fills.extend(fill_of(ops)),
                _ => {}
            }
        }
        if !fills.is_empty() {
            out.insert(sector, (values, fills));
        }
    }
    out
}

/// A `FillSector` record's count (`35`) and entries (`19:(type, weight, hostile)`, or a bare `02`
/// type of weight 100, peaceful), its `08` or `0E` making every entry hostile or peaceful.
fn fill_of(ops: &[opensacred_data::netscript::Operand]) -> Option<spawning::Fill> {
    let per_area = ops.iter().find_map(|o| match (o.tag, &o.value) {
        (0x35, Value::I32x3([a, b, area])) => Some((
            u32::try_from(*a).ok()?,
            u32::try_from(*b).ok()?,
            u32::try_from(*area).ok()?,
        )),
        _ => None,
    })?;
    let forced = ops
        .iter()
        .filter_map(|o| match (o.tag, &o.value) {
            (0x08, Value::Marker) => Some(true),
            (0x0E, Value::Marker) => Some(false),
            _ => None,
        })
        .next_back();
    let entries = ops
        .iter()
        .filter_map(|o| match (o.tag, &o.value) {
            (0x19, Value::I32x3([creature, weight, hostile])) => Some(spawning::Entry {
                creature: u32::try_from(*creature).ok()?,
                weight: u32::try_from(*weight).ok()?,
                hostile: *hostile != 0,
            }),
            (0x02, Value::I32(creature)) => Some(spawning::Entry {
                creature: u32::try_from(*creature).ok()?,
                weight: 100,
                hostile: false,
            }),
            _ => None,
        })
        .map(|entry| spawning::Entry {
            hostile: forced.unwrap_or(entry.hostile),
            ..entry
        })
        .collect();
    Some(spawning::Fill { per_area, entries })
}

/// The balance's factors of the fills' density (`SpawnAnimalsSP`, `SpawnEnemysSP`), 0.8 and 1.2
/// where `tables/balance.ron` lacks them.
fn factors(assets: &Path) -> spawning::Factors {
    let balance = fs::read_to_string(assets.join("tables").join(opensacred_data::tables::BALANCE))
        .ok()
        .and_then(|text| ron::from_str::<opensacred_data::tables::Balance>(&text).ok());
    let factor = |name, default| {
        balance
            .as_ref()
            .and_then(|b| b.f32(name))
            .unwrap_or(default)
    };
    spawning::Factors {
        animals: factor("SpawnAnimalsSP", 0.8),
        enemies: factor("SpawnEnemysSP", 1.2),
    }
}

/// The sector of a function of the sectors', by its name, `Sector<x><yyy>Init` or `…Enter` as the
/// engine makes it (`Sector%d%3.3dInit`, `0x94F088`); `None` for any other function, and for the
/// few named another way (`Sector28Enter`), which the engine never asks for.
fn sector_function(name: &str) -> Option<(u32, u32)> {
    let digits = name.strip_prefix("Sector")?;
    let digits = digits
        .strip_suffix("Init")
        .or_else(|| digits.strip_suffix("Enter"))?;
    if digits.len() < 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (x, y) = digits.split_at(digits.len() - 3);
    Some((x.parse().ok()?, y.parse().ok()?))
}

/// A record's name (`0x01`), lower case: an NPC's, which `CreateNPC` gives it and `NPC_Goto`
/// names it by.
fn name_of(ops: &[opensacred_data::netscript::Operand]) -> Option<String> {
    ops.iter().find_map(|o| match (o.tag, &o.value) {
        (0x01, Value::Str(name)) => Some(name.to_ascii_lowercase()),
        _ => None,
    })
}

/// Where the records' `NPC_Goto`s send the NPCs they name, by name: the global cell and the
/// layer of the last that names each (`0x49E210`, which orders the creature to walk there).
fn goto_targets(records: &[&Record]) -> HashMap<String, (i32, i32, i32)> {
    records
        .iter()
        .filter(|r| r.kind == NPC_GOTO)
        .filter_map(|r| {
            let Body::Operands(ops) = &r.body else {
                return None;
            };
            let target = ops.iter().find_map(|o| match (o.tag, &o.value) {
                (0x04, Value::I32x3([x, y, layer])) => Some((*x, *y, *layer)),
                _ => None,
            })?;
            Some((name_of(ops)?, target))
        })
        .collect()
}

/// The cells, global and their floors, that the records' `TriggerPatch`es (`0x496F20`) put under
/// a trigger that a `CreateTrigger` (`0x496520`) made closed: its tag [`CLOSED`] clears its
/// state's open bit, [`OPEN`] sets it, the last of them counting (a trigger of neither is open,
/// **hypothesis**, as the sector files' are). The client's cell test (`0x4EBCA0`) refuses such a
/// cell to every creature, the chests' and the barrels' among them (docs/re/movement.md, "The
/// chests' cells"); no script of the start opens them.
fn closed_cells(records: &[&Record]) -> Vec<Spot> {
    let ops = |kind| {
        records
            .iter()
            .filter(move |r| r.kind == kind)
            .filter_map(|r| match &r.body {
                Body::Operands(ops) => Some(ops),
                _ => None,
            })
    };
    let closed: HashSet<String> = ops(CREATE_TRIGGER)
        .filter(|ops| {
            ops.iter()
                .filter_map(|o| match (o.tag, &o.value) {
                    (OPEN | CLOSED, Value::Marker) => Some(o.tag == CLOSED),
                    _ => None,
                })
                .next_back()
                .unwrap_or(false)
        })
        .filter_map(|ops| name_of(ops))
        .collect();
    ops(TRIGGER_PATCH)
        .filter(|ops| name_of(ops).is_some_and(|name| closed.contains(&name)))
        .flat_map(|ops| {
            ops.iter().filter_map(|o| match (o.tag, &o.value) {
                (0x2A, Value::I32x3([x, y, floor])) => Some((*x, *y, *floor)),
                _ => None,
            })
        })
        .collect()
}

/// The named positions of the start script: `DefPos` name, x, y.
fn positions<'a>(records: impl IntoIterator<Item = &'a Record>) -> HashMap<String, (i32, i32)> {
    records
        .into_iter()
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
        let path = assets
            .join("tables")
            .join(opensacred_data::tables::CREATURES);
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let creatures: Vec<Creature> =
            ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        let shadowless: HashSet<u32> = creatures
            .iter()
            .filter(|c| c.flags & NO_SHADOW != 0)
            .map(|c| c.id)
            .collect();
        let record_flags: HashMap<u32, u8> = creatures.iter().map(|c| (c.id, c.flags)).collect();
        // Each creature's walk and run speeds (`+0x26`, `+0x28`, the models' units a second).
        let speeds: HashMap<u32, [u16; 2]> = creatures.iter().map(|c| (c.id, c.speed)).collect();
        let path = assets.join("tables").join(opensacred_data::tables::WEAPONS);
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let weapons: Vec<Weapon> =
            ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        // A weapon's kind, by its type, which is its item's slot (docs/02 §6.2).
        let kinds: HashMap<u32, u8> = weapons.iter().map(|w| (w.id, w.bytes_1c[2])).collect();
        let models = model_files(assets)?;
        let textures = texture_files(assets)?;
        let records = start_records(&tree);
        let named = positions(records.iter().copied());
        let targets = goto_targets(&records);
        // The NPCs' models' clips by their slots, each model read once.
        let mut model_slots: HashMap<String, motions::Slots> = HashMap::new();
        let mut placements = Vec::new();
        for record in records
            .iter()
            .filter(|r| r.kind == CREATE_NPC || r.kind == CREATE_OBJ)
        {
            let Body::Operands(ops) = &record.body else {
                continue;
            };
            let mut items_named = ops.iter().filter_map(|o| match (o.tag, &o.value) {
                (0x02, Value::I32(v)) => u32::try_from(*v).ok(),
                _ => None,
            });
            let creature = items_named.next();
            let equipped: Vec<&Item> = items_named
                .filter_map(|slot| items.get(&slot).copied())
                .collect();
            let categories: Vec<(u8, Option<u8>)> = equipped
                .iter()
                .map(|item| (category(item), kinds.get(&item.slot).copied()))
                .collect();
            let equipment: Vec<Held> = equipped
                .iter()
                .zip(hands(&categories))
                .filter_map(|(item, hand)| {
                    let model = models.get(&item.name.to_ascii_uppercase())?;
                    let texture = match item.words_08[0] {
                        0 => None,
                        number => textures.get(&number).cloned(),
                    };
                    Some(Held {
                        model: model.clone(),
                        texture,
                        hand: hand?,
                        shield: category(item) == SHIELD,
                    })
                })
                .collect();
            // An NPC sent somewhere at the start walks there from where it was made.
            let target = name_of(ops).and_then(|name| targets.get(&name).copied());
            let cell = position(ops, &named).or(target);
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
            // An NPC's clips standing and walking, by the class of what it holds; walking at a
            // run where its walk is a run.
            let (clips, gait) = if record.kind == CREATE_NPC {
                if !model_slots.contains_key(model) {
                    let slots = motions::Slots::read(&assets.join(model))?;
                    model_slots.insert(model.clone(), slots);
                }
                let slots = &model_slots[model];
                let class = held_class(&categories);
                let stand = slots.clip(motions::Action::Stand, class);
                let walk = slots.clip(motions::Action::Walk, class);
                let clips = ClipNames {
                    stand: stand.map(|(name, _)| name.to_owned()),
                    walk: walk.map(|(name, _)| name.to_owned()),
                    stance: slots.stance(class).map(str::to_owned),
                };
                (clips, walk.map(|(_, gait)| gait))
            } else {
                (ClipNames::default(), None)
            };
            let walk = target
                .filter(|_| record.kind == CREATE_NPC)
                .and_then(|(tx, ty, _)| Some((u32::try_from(tx).ok()?, u32::try_from(ty).ok()?)))
                .filter(|&to| to != (x, y))
                .map(|to| {
                    let [walking, running] = speeds.get(&creature).copied().unwrap_or_default();
                    let speed = if gait == Some(motions::Action::Run) {
                        running
                    } else {
                        walking
                    };
                    (to, f32::from(speed) * iso::MODEL_SCALE)
                });
            placements.push(Placement {
                model: model.clone(),
                cell: (x, y),
                walk,
                heading: facing_degrees(record.kind == CREATE_OBJ, heading, item.angle()),
                object: record.kind == CREATE_OBJ,
                casts: if record.kind == CREATE_OBJ {
                    category(item) == CHEST
                } else {
                    !shadowless.contains(&creature)
                },
                blocks: record.kind == CREATE_OBJ && category(item) == CHEST,
                creature: (
                    creature,
                    record_flags.get(&creature).copied().unwrap_or_default(),
                    crowd::flags_word(
                        ops.iter()
                            .filter(|o| matches!(o.value, Value::Marker))
                            .map(|o| o.tag),
                    ),
                ),
                texture,
                layer,
                equipment: if record.kind == CREATE_NPC {
                    equipment
                } else {
                    Vec::new()
                },
                clips,
            });
        }
        // What each type that the sectors' fills make is placed as: its model standing with its
        // hands empty, its shadow, and its flags word, to which a hostile entry adds bit 0.
        let fills = fills(&tree);
        let mut templates: HashMap<u32, Placement> = HashMap::new();
        for entry in fills
            .values()
            .flat_map(|(_, fills)| fills)
            .flat_map(|fill| &fill.entries)
        {
            let creature = entry.creature;
            if templates.contains_key(&creature) {
                continue;
            }
            let Some(item) = items.get(&creature) else {
                continue;
            };
            let Some(model) = models.get(&item.name.to_ascii_uppercase()) else {
                continue;
            };
            if !model_slots.contains_key(model) {
                let slots = motions::Slots::read(&assets.join(model))?;
                model_slots.insert(model.clone(), slots);
            }
            let slots = &model_slots[model];
            let clip = |action| {
                slots
                    .clip(action, held_class(&[]))
                    .map(|(name, _)| name.to_owned())
            };
            templates.insert(
                creature,
                Placement {
                    model: model.clone(),
                    cell: (0, 0),
                    walk: None,
                    heading: 0.0,
                    object: false,
                    casts: !shadowless.contains(&creature),
                    blocks: false,
                    creature: (
                        creature,
                        record_flags.get(&creature).copied().unwrap_or_default(),
                        0,
                    ),
                    texture: match item.words_08[0] {
                        0 => None,
                        number => textures.get(&number).cloned(),
                    },
                    layer: 0,
                    equipment: Vec::new(),
                    clips: ClipNames {
                        stand: clip(motions::Action::Stand),
                        walk: clip(motions::Action::Walk),
                        stance: slots.stance(held_class(&[])).map(str::to_owned),
                    },
                },
            );
        }
        let classes = creatures.iter().map(|c| (c.id, c.class)).collect();
        let side = u32::try_from(SIDE)?;
        let mut by_sector: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (index, placement) in placements.iter().enumerate() {
            let (x, y) = placement.cell;
            by_sector
                .entry((x / side, y / side))
                .or_default()
                .push(index);
        }
        let mut closed: HashMap<(u32, u32), Vec<Spot>> = HashMap::new();
        for (x, y, floor) in closed_cells(&records) {
            if let (Ok(cx), Ok(cy)) = (u32::try_from(x), u32::try_from(y)) {
                closed
                    .entry((cx / side, cy / side))
                    .or_default()
                    .push((x, y, floor));
            }
        }
        Ok(Self {
            placements,
            by_sector,
            closed,
            fills,
            templates,
            classes,
            factors: factors(assets),
            spawned: HashSet::new(),
        })
    }

    /// The NPCs and the objects to place, and the cells of the closed triggers.
    pub fn counts(&self) -> (usize, usize, usize) {
        let objects = self.placements.iter().filter(|p| p.object).count();
        let closed = self.closed.values().map(Vec::len).sum();
        (self.placements.len() - objects, objects, closed)
    }
}

/// Places the NPCs of every sector that came in, as entities of that sector, and marks the cells
/// that its objects take.
pub fn place(
    mut commands: Commands,
    mut npcs: ResMut<Npcs>,
    mut streamer: ResMut<Streamer>,
    mut heights: ResMut<Heights>,
    states: Res<buildings::Buildings>,
    assets: Res<AssetServer>,
) {
    let npcs = &mut *npcs;
    let loaded: Vec<(u32, u32)> = streamer.sectors().collect();
    for &at in &loaded {
        if npcs.spawned.contains(&at) {
            continue;
        }
        npcs.spawned.insert(at);
        // The closed triggers' cells: of the ground for the floors 0 and 1, of the building's
        // floor for 1 and above (`0x463F50`, which keys the floors 0 and 1 alike).
        for &(x, y, floor) in npcs.closed.get(&at).map(Vec::as_slice).unwrap_or_default() {
            if floor <= 1 {
                heights.occupy((x, y), None);
            }
            if let Ok(floor) = usize::try_from(floor)
                && floor >= 1
                && let Some(building) = heights.building_on_floor((x, y), floor)
                && let Some(level) = heights.floor_level(building, floor)
            {
                heights.occupy((x, y), Some(level));
            }
        }
        for &index in npcs
            .by_sector
            .get(&at)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            let entity = spawn(
                &mut commands,
                &npcs.placements[index],
                &heights,
                &states,
                &assets,
            );
            streamer.adopt(at, entity);
        }
        // The creatures that the sector's fills make, on cells at random that they may stand
        // on, the same ones each time the sector comes in (`spawning`).
        let Some((values, fills)) = npcs.fills.get(&at) else {
            continue;
        };
        let (Ok(x0), Ok(y0)) = (i32::try_from(at.0 * 64), i32::try_from(at.1 * 64)) else {
            continue;
        };
        for (index, fill) in fills.iter().enumerate() {
            let mut rng = spawning::Rng::of(at.0, at.1, index);
            let first = fill
                .entries
                .first()
                .and_then(|entry| npcs.classes.get(&entry.creature).copied());
            let factor = if spawning::animals(first) {
                npcs.factors.animals
            } else {
                npcs.factors.enemies
            };
            for _ in 0..spawning::count(fill, *values, HERO_LEVEL, factor, &mut rng) {
                let Some(entry) = spawning::choose(&fill.entries, &mut rng) else {
                    break;
                };
                let Some(cell) = spawning::spot(
                    (x0, y0),
                    &mut rng,
                    |cell| heights.walkable_on(cell, None),
                    |cell| heights.flags(cell),
                ) else {
                    continue;
                };
                let (Some(template), Ok(cx), Ok(cy)) = (
                    npcs.templates.get(&entry.creature),
                    u32::try_from(cell.0),
                    u32::try_from(cell.1),
                ) else {
                    continue;
                };
                let mut placement = template.clone();
                placement.cell = (cx, cy);
                // A random heading, as `CreateNPC` gives one without its own (`rand() % 361`).
                placement.heading = f32::from(u16::try_from(rng.below(361)).unwrap_or(0));
                if entry.hostile {
                    placement.creature.2 |= crowd::HOSTILE;
                }
                let entity = spawn(&mut commands, &placement, &heights, &states, &assets);
                streamer.adopt(at, entity);
            }
        }
    }
    // Sectors that went out place their NPCs again when they come back.
    npcs.spawned.retain(|at| loaded.contains(at));
}

/// Spawns the NPC or the object of `placement`: its model on its cell and level, turned to its
/// heading, with its texture, its items, its shadow, its clips, its walk and its place in the
/// others' way.
fn spawn(
    commands: &mut Commands,
    placement: &Placement,
    heights: &Heights,
    states: &buildings::Buildings,
    assets: &AssetServer,
) -> Entity {
    let (x, y) = placement.cell;
    let mut position = iso::ground(x as f32 + 0.5, y as f32 + 0.5);
    // On a building's floor: the building whose floor's grid holds the cell, or the
    // cell's on the ground; on its level's heights, a floor's at the floor's height.
    let floor = usize::try_from(placement.layer).ok().filter(|&f| f > 0);
    let on_floor = match (i32::try_from(x), i32::try_from(y), floor) {
        (Ok(cx), Ok(cy), Some(floor)) => heights.building_on_floor((cx, cy), floor),
        _ => None,
    };
    let level = match (on_floor, floor) {
        (Some(building), Some(floor)) => heights.floor_level(building, floor),
        _ => None,
    };
    position.y = heights
        .standing(position.x, position.z, level)
        .unwrap_or(0.0);
    // In its layer of the building whose floor or cell it is on, if any.
    let layered = buildings::Layered::of(
        heights,
        on_floor.or_else(|| {
            heights
                .building(position.x, position.z)
                .map(|(building, _)| building)
        }),
        placement.layer,
    );
    let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(placement.model.clone()));
    let mut entity = commands.spawn((
        WorldAssetRoot(scene),
        Transform::from_translation(position)
            .with_rotation(hero::facing(heading(placement.heading)))
            .with_scale(Vec3::splat(iso::MODEL_SCALE)),
        models::Ordered::default(),
        layered,
        layered.visibility(states),
    ));
    if let Some(texture) = &placement.texture {
        entity.insert(Retexture(load_texture(assets, texture)));
    }
    if !placement.equipment.is_empty() {
        entity.insert(Equipment(
            placement
                .equipment
                .iter()
                .map(|held| Equip {
                    gltf: assets.load(held.model.clone()),
                    texture: held.texture.as_ref().map(|t| load_texture(assets, t)),
                    hand: held.hand,
                    shield: held.shield,
                    ready: false,
                })
                .collect(),
        ));
    }
    if placement.casts {
        entity.insert(shadows::Casts);
    }
    if placement.object {
        entity.insert(Object);
        if placement.blocks {
            entity.insert(crowd::InTheWay::object(level));
        }
    } else {
        let (creature, record_flags, word) = placement.creature;
        entity.insert((
            Npc {
                gltf: assets.load(placement.model.clone()),
                names: placement.clips.clone(),
                graph: None,
                player: None,
            },
            crowd::InTheWay::of(creature, record_flags, word, level),
        ));
        if let Some(((tx, ty), speed)) = placement.walk {
            entity.insert(Walk {
                target: iso::ground(tx as f32 + 0.5, ty as f32 + 0.5),
                level,
                speed,
                climbed: false,
            });
        }
    }
    entity.id()
}

/// An item's texture, repeated, as a glTF texture without a sampler is: the models' texture
/// coordinates run past the image (the lid of `chest.grn` from -0.99 to -0.36).
fn load_texture(assets: &AssetServer, texture: &str) -> Handle<Image> {
    assets
        .load_builder()
        .with_settings(|settings: &mut ImageLoaderSettings| {
            let mut sampler = ImageSamplerDescriptor::linear();
            sampler.set_address_mode(ImageAddressMode::Repeat);
            settings.sampler = ImageSampler::Descriptor(sampler);
        })
        .load(texture.to_owned())
}

/// An item's category (`items.pak +0x2E`, the third byte of the word at `+0x2C`).
fn category(item: &Item) -> u8 {
    item.words_14[6].to_le_bytes()[2]
}

/// An NPC's equipment still to attach: each item's model, its texture, its hand, and whether the
/// NPC's own scene is ready.
#[derive(Component)]
pub struct Equipment(Vec<Equip>);

pub struct Equip {
    gltf: Handle<Gltf>,
    texture: Option<Handle<Image>>,
    hand: Hand,
    shield: bool,
    ready: bool,
}

/// The bones that join an item to its holder (`0x555300`, `0x401AE0`): the item's socket for its
/// hand, the holder's own socket of that name if its skeleton has one (the heroes' do, under
/// their hands), else its hand, and for a shield always its forearm.
fn bones(hand: Hand, shield: bool) -> (&'static str, Option<&'static str>, &'static str) {
    match (hand, shield) {
        (Hand::Left, false) => ("Bone_weapon_02", Some("Bone_weapon_02"), "Bip01 L Hand"),
        (Hand::Right, false) => ("Bone_weapon_01", Some("Bone_weapon_01"), "Bip01 R Hand"),
        (Hand::Left, true) => ("Bone_weapon_02", None, "Bip01 L Forearm"),
        (Hand::Right, true) => ("Bone_weapon_01", None, "Bip01 R Forearm"),
    }
}

/// Marks an NPC's equipment as ready to attach once its scene has spawned.
pub fn equipment_ready(ready: On<WorldInstanceReady>, mut npcs: Query<&mut Equipment>) {
    if let Ok(mut equipment) = npcs.get_mut(ready.entity) {
        for item in &mut equipment.0 {
            item.ready = true;
        }
    }
}

/// Attaches each NPC's equipment, once its scene is ready and the item's glTF is loaded: the
/// item's socket bone (see [`bones`]) goes onto the NPC's bone that holds it. An item whose model
/// lacks its socket is not drawn.
pub fn equip(
    mut commands: Commands,
    mut npcs: Query<(Entity, &mut Equipment)>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    for (npc, mut equipment) in &mut npcs {
        equipment.0.retain(|item| {
            if !item.ready {
                return true;
            }
            let Some(gltf) = gltfs.get(&item.gltf) else {
                return true;
            };
            let (socket, own, holder) = bones(item.hand, item.shield);
            let Some(pose) = pose(gltf, &nodes, socket) else {
                return false;
            };
            let named = |wanted: &str| {
                children
                    .iter_descendants(npc)
                    .find(|&e| names.get(e).is_ok_and(|n| n.as_str() == wanted))
            };
            let Some(holder) = own.and_then(named).or_else(|| named(holder)) else {
                return false;
            };
            let Some(scene) = gltf.scenes.first().cloned() else {
                return false;
            };
            let mut weapon = commands.spawn((
                WorldAssetRoot(scene),
                Transform::from_matrix(pose.to_matrix().inverse()),
                ChildOf(holder),
                models::OrderedWith(npc),
            ));
            if let Some(texture) = &item.texture {
                weapon.insert(Retexture(texture.clone()));
            }
            false
        });
        if equipment.0.is_empty() {
            commands.entity(npc).remove::<Equipment>();
        }
    }
}

/// The pose of the node named `name` in a glTF's scene, from its root: its position and
/// orientation alone. The scale of the sockets takes no part: the dark elves' blades have sockets
/// of scale 0.23 and meshes as long as a sword's (**hypothesis**: the client's attach takes the
/// components of mask `0x3F`, `0x44C670`, which by these models are the position and the
/// orientation).
fn pose(gltf: &Gltf, nodes: &Assets<GltfNode>, name: &str) -> Option<Transform> {
    let all: Vec<&GltfNode> = gltf
        .nodes
        .iter()
        .filter_map(|node| nodes.get(node))
        .collect();
    let children: HashSet<usize> = all
        .iter()
        .flat_map(|node| node.children.iter().filter_map(|child| nodes.get(child)))
        .map(|child| child.index)
        .collect();
    let mut stack: Vec<(&GltfNode, Transform)> = all
        .into_iter()
        .filter(|node| !children.contains(&node.index))
        .map(|node| (node, Transform::IDENTITY))
        .collect();
    while let Some((node, parent)) = stack.pop() {
        let local = Transform::from_translation(node.transform.translation)
            .with_rotation(node.transform.rotation);
        let pose = parent * local;
        if node.name == name {
            return Some(pose);
        }
        stack.extend(
            node.children
                .iter()
                .filter_map(|child| nodes.get(child))
                .map(|child| (child, pose)),
        );
    }
    None
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

/// Gives each NPC's animation player its clips (`ClipNames`): the one it stands in, else its
/// model's first, and its walk clip, which plays while the NPC walks.
pub fn animate(
    mut commands: Commands,
    mut npcs: Query<(&mut Npc, Has<Walk>)>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut players: Query<(Entity, &mut AnimationPlayer), Without<AnimationGraphHandle>>,
    parents: Query<&ChildOf>,
    speed: Res<hero::GameSpeed>,
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
        let Some((mut npc, walking)) = npc else {
            continue;
        };
        if npc.graph.is_none() {
            let Some(gltf) = gltfs.get(&npc.gltf) else {
                continue;
            };
            let clip = |name: &Option<String>| {
                name.as_deref()
                    .and_then(|name| gltf.named_animations.get(name))
            };
            let Some(idle) = clip(&npc.names.stand)
                .or_else(|| gltf.animations.first())
                .cloned()
            else {
                continue;
            };
            let walk = clip(&npc.names.walk);
            let stance = clip(&npc.names.stance);
            let mut graph = AnimationGraph::new();
            let root = graph.root;
            let idle = graph.add_clip(idle, 1.0, root);
            let walk = walk.map(|clip| graph.add_clip(clip.clone(), 1.0, root));
            let stance = stance.map(|clip| graph.add_clip(clip.clone(), 1.0, root));
            npc.graph = Some(Clips {
                graph: graphs.add(graph),
                idle,
                walk,
                stance,
            });
        }
        if let Some(clips) = npc.graph.clone() {
            let node = match (walking, clips.walk) {
                (true, Some(walk)) => walk,
                _ => clips.idle,
            };
            // The clips change through transitions, which blend them (`stance`).
            let mut transitions = AnimationTransitions::new();
            transitions
                .play(&mut player, node, Duration::ZERO)
                .repeat()
                .set_speed(speed.0);
            commands
                .entity(entity)
                .insert((AnimationGraphHandle(clips.graph), transitions));
            npc.player = Some(entity);
        }
    }
}

/// Walks each walking NPC toward its target as the hero walks (`hero::run`): round what blocks
/// the way and through the doors, at its walk speed times the game's, facing its way; at the
/// target it stands, its idle clip playing. A target it cannot reach it walks straight toward,
/// up to what blocks it, and stands there.
pub fn walk(
    mut commands: Commands,
    time: Res<Time>,
    heights: Res<Heights>,
    crowd: Res<crowd::Crowd>,
    speed: Res<hero::GameSpeed>,
    mut walkers: Query<(
        Entity,
        &mut Transform,
        &mut Walk,
        &mut buildings::Layered,
        &mut crowd::InTheWay,
        &Npc,
    )>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for (entity, mut transform, mut walk, mut layered, mut way, npc) in &mut walkers {
        let mut here = transform.translation;
        let cell = paths::cell_of(here.x, here.z);
        if walk.level.is_none() && !heights.walkable(cell) {
            walk.level = heights.level_of(cell);
        }
        walk.level = heights.transition(cell, walk.level);
        // Up or down a stair it stepped onto from off the stairs (`0x5FCCB0`).
        if !heights.on_stairs(cell, walk.level) {
            walk.climbed = false;
        }
        if !walk.climbed
            && let Some((to, level)) = heights.climb(cell, walk.level)
        {
            here += iso::ground((to.0 - cell.0) as f32, (to.1 - cell.1) as f32).with_y(0.0);
            if let Some(y) = heights.standing(here.x, here.z, level) {
                here.y = y;
            }
            transform.translation = here;
            walk.level = level;
            walk.climbed = true;
        }
        // In the layer of its level, at the cell it stands on: it stays there once it stands.
        layered.set_if_neq(buildings::Layered::at(&heights, here.x, here.z, walk.level));
        way.level = walk.level;
        let level = walk.level;
        let target = walk.target;
        // A cell it may step into: walkable, no object and no other creature in it (`0x4F7E10`:
        // an NPC, of an id above 16, is stopped by any).
        let free =
            |cell, at| heights.walkable_on(cell, at) && !crowd.stops(cell, at, entity, false);
        let toward = match paths::next(
            here,
            (level, false),
            target,
            None,
            |cell, (at, _)| free(cell, at),
            |place| {
                let mut next = heights.neighbours(place);
                next.retain(|&(cell, (at, _))| !crowd.stops(cell, at, entity, false));
                next
            },
            |(at, _)| at,
        ) {
            Some(paths::Step::Through((next, _))) => {
                walk.level = next;
                continue;
            }
            Some(paths::Step::Toward(point)) => point,
            None => target,
        };
        let to = (toward - here).with_y(0.0);
        let distance = to.length();
        let left = (target - here).with_y(0.0).length();
        let step = (walk.speed * speed.0 * time.delta_secs()).min(distance);
        let mut position = here + to / distance.max(f32::EPSILON) * step;
        let (from, into) = (cell, paths::cell_of(position.x, position.z));
        let blocked =
            into != from && !free(into, walk.level) && heights.walkable_on(from, walk.level);
        if left <= hero::ARRIVED || blocked || distance <= f32::EPSILON {
            // There, or as near as it gets: it stands.
            commands.entity(entity).remove::<Walk>();
            if let Some(clips) = &npc.graph
                && let Some((mut player, mut transitions)) =
                    npc.player.and_then(|p| players.get_mut(p).ok())
            {
                transitions
                    .play(&mut player, clips.idle, BLEND)
                    .repeat()
                    .set_speed(speed.0);
            }
            continue;
        }
        // On its level's heights: a bridge's deck, a building's floor.
        if let Some(y) = heights.standing(position.x, position.z, walk.level) {
            position.y = y;
        }
        transform.translation = position;
        transform.rotation = hero::facing(to);
    }
}

/// How long a standing creature's clip blends into the next (`0x593320`).
const BLEND: Duration = Duration::from_millis(150);
/// How near the hero's nearest hostile puts the standing creatures in their fight stance, in
/// world units (`0x546A7E`).
const STANCE_WITHIN: f32 = 550.0;

/// Puts each standing NPC whose model has a fight stance into it while the hero's nearest hostile
/// is within 550 units, wherever the NPC stands, and back into its idle otherwise, blending over
/// 0.15 s (`0x5467A0`, which a standing creature asks at every update, `0x544890`;
/// docs/re/motions.md, "The fight stance").
pub fn stance(
    nearest: Res<crowd::Nearest>,
    npcs: Query<&Npc, Without<Walk>>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    speed: Res<hero::GameSpeed>,
) {
    let fight = nearest.distance < STANCE_WITHIN;
    for npc in &npcs {
        let (Some(clips), Some(player)) = (&npc.graph, npc.player) else {
            continue;
        };
        let wanted = match (fight, clips.stance) {
            (true, Some(stance)) => stance,
            _ => clips.idle,
        };
        let Ok((mut player, mut transitions)) = players.get_mut(player) else {
            continue;
        };
        if transitions.get_main_animation() != Some(wanted) {
            transitions
                .play(&mut player, wanted, BLEND)
                .repeat()
                .set_speed(speed.0);
        }
    }
}

/// The start script's NPCs and objects: placed as their sectors come in, animated, walked,
/// retextured and equipped.
pub struct NpcsPlugin;

impl Plugin for NpcsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Retextured>()
            .add_observer(retexture)
            .add_observer(equipment_ready)
            .add_systems(
                Update,
                (place, animate, walk, stance, equip)
                    .chain()
                    .in_set(Step::Npcs),
            );
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::Vec2;
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
    fn items_go_in_the_hands_the_client_gives_them() {
        use Hand::{Left, Right};
        // A soldier's sword and shield, a crossbow, a dark elf's two blades, a third blade.
        assert_eq!(
            hands(&[(WEAPON, Some(1)), (SHIELD, Some(0))]),
            [Some(Right), Some(Left)]
        );
        assert_eq!(hands(&[(WEAPON, Some(CROSSBOW))]), [Some(Left)]);
        assert_eq!(
            hands(&[(WEAPON, Some(BOW)), (WEAPON, Some(1))]),
            [Some(Left), Some(Right)]
        );
        assert_eq!(
            hands(&[(WEAPON, Some(0)), (WEAPON, None), (WEAPON, Some(10))]),
            [Some(Right), Some(Left), None]
        );
        // A second shield finds its hand taken; a ring goes in no hand.
        assert_eq!(
            hands(&[(SHIELD, None), (SHIELD, None), (8, None)]),
            [Some(Left), None, None]
        );
    }

    #[test]
    fn a_heading_is_an_angle_on_the_screen() {
        let close = |a: Vec3, b: Vec3| (a - b).length() < 1e-6;
        // To the right on the screen, between the grid's x and its −y; down, between x and y.
        assert!(close(heading(0.0), Vec3::new(1.0, 0.0, -1.0).normalize()));
        assert!(close(heading(90.0), Vec3::new(1.0, 0.0, 1.0).normalize()));
        // At its angle on the screen, clockwise from the right: a chest of heading 20 shows its
        // long side almost level, one of 330 up and to the right.
        for degrees in [0.0_f32, 20.0, 45.0, 135.0, 200.0, 330.0] {
            let direction = heading(degrees);
            let on_screen = Vec2::new(direction.dot(iso::right()), -direction.dot(iso::up()));
            let (sin, cos) = degrees.to_radians().sin_cos();
            assert!(
                (on_screen.normalize() - Vec2::new(cos, sin)).length() < 1e-5,
                "{degrees}: {on_screen}"
            );
        }
    }

    #[test]
    fn an_object_without_a_heading_faces_its_items_angle() {
        // `chest.grn`'s −30: without a heading, or with 0, which `CreateObj` leaves alone.
        assert_eq!(facing_degrees(true, None, -30.0), -30.0);
        assert_eq!(facing_degrees(true, Some(0.0), -30.0), -30.0);
        assert_eq!(facing_degrees(true, Some(20.0), -30.0), 20.0);
        // An NPC takes its heading, 0 without one.
        assert_eq!(facing_degrees(false, Some(45.0), 90.0), 45.0);
        assert_eq!(facing_degrees(false, None, 90.0), 0.0);
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

    #[test]
    fn an_npc_picks_its_clips_by_what_it_holds() {
        // A soldier's sword (1) and kite shield; a crossbow (12); a thief's dagger (5); a dark
        // elf's two blades; a weapon without a `Weapon.pak` record; a ring alone.
        assert_eq!(held_class(&[(WEAPON, Some(1)), (SHIELD, Some(0))]), 1);
        assert_eq!(held_class(&[(WEAPON, Some(CROSSBOW))]), 12);
        assert_eq!(held_class(&[(WEAPON, Some(5))]), 5);
        assert_eq!(held_class(&[(WEAPON, Some(9)), (WEAPON, Some(9))]), 8);
        assert_eq!(held_class(&[(WEAPON, None)]), 0);
        assert_eq!(held_class(&[(8, None)]), 0);
    }

    #[test]
    fn the_quests_the_start_enters_place_their_npcs() {
        let op = |tag, value| Operand { tag, value };
        let record = |kind, ops| Record {
            kind,
            body: Body::Operands(ops),
        };
        let quest = |kind, n| record(kind, vec![op(0x0B, Value::I32(n))]);
        let function = |name: &str, records| Function {
            name: name.into(),
            quest: 1,
            region: 0,
            records,
        };
        let novice = || {
            record(
                CREATE_NPC,
                vec![
                    op(0x01, Value::Str("res:17095".into())),
                    op(0x02, Value::I32(679)),
                    op(0x04, Value::Name("novizin1".into())),
                ],
            )
        };
        let tree = Tree {
            start: vec![
                // Quest 1 set up and triggered; quest 2 triggered, not set up; quest 3 set up and
                // triggered, but its trigger has a condition.
                quest(SET_UP_QUEST, 1),
                quest(TRIGGER_QUEST, 1),
                quest(TRIGGER_QUEST, 2),
                quest(SET_UP_QUEST, 3),
                quest(TRIGGER_QUEST, 3),
            ],
            quest: Vec::new(),
            functions: vec![
                function("QIS_Trigger1", Vec::new()),
                function(
                    "QIS_OnEnter1",
                    vec![
                        novice(),
                        record(
                            NPC_GOTO,
                            vec![
                                op(0x01, Value::Str("Res:17095".into())),
                                op(0x04, Value::I32x3([3237, 2514, 0])),
                            ],
                        ),
                    ],
                ),
                function("QIS_Trigger2", Vec::new()),
                function("QIS_OnEnter2", vec![novice()]),
                function("QIS_Trigger3", vec![quest(TRIGGER_QUEST, 9)]),
                function("QIS_OnEnter3", vec![novice()]),
            ],
            quests: Vec::new(),
            regions: Vec::new(),
        };
        assert_eq!(entered_quests(&tree), [1]);
        let records = start_records(&tree);
        assert_eq!(records.len(), 7);
        assert_eq!(
            records.iter().filter(|r| r.kind == CREATE_NPC).count(),
            1,
            "quest 1's novice alone"
        );
        // She stands where the quest sends her, whatever the case of her name.
        let targets = goto_targets(&records);
        assert_eq!(targets["res:17095"], (3237, 2514, 0));
    }

    #[test]
    fn the_cells_patched_under_a_closed_trigger_are_closed() {
        let op = |tag, value| Operand { tag, value };
        let record = |kind, ops| Record {
            kind,
            body: Body::Operands(ops),
        };
        let name = |name: &str| op(0x01, Value::Str(name.into()));
        let create = |trigger: &str, tags: &[u8]| {
            let mut ops = vec![name(trigger), op(0x28, Value::I32(0))];
            ops.extend(tags.iter().map(|&tag| op(tag, Value::Marker)));
            record(CREATE_TRIGGER, ops)
        };
        let patch = |trigger: &str, cells: &[[i32; 3]]| {
            let mut ops = vec![name(trigger)];
            ops.extend(cells.iter().map(|&cell| op(0x2A, Value::I32x3(cell))));
            record(TRIGGER_PATCH, ops)
        };
        let records = [
            // The chests' trigger, closed; a door's, closed then opened; one of neither.
            create("chestTrigger", &[CLOSED]),
            create("door", &[CLOSED, OPEN]),
            create("plain", &[]),
            // A wide chest's three cells on floor 1, and a patch of each other trigger.
            patch(
                "chestTrigger",
                &[[3341, 2512, 1], [3342, 2512, 1], [3343, 2512, 1]],
            ),
            patch("door", &[[3300, 2500, 0]]),
            patch("plain", &[[3301, 2500, 0]]),
            patch("CHESTTRIGGER", &[[3200, 2490, 0]]),
        ];
        let records: Vec<&Record> = records.iter().collect();
        assert_eq!(
            closed_cells(&records),
            [
                (3341, 2512, 1),
                (3342, 2512, 1),
                (3343, 2512, 1),
                (3200, 2490, 0)
            ]
        );
    }

    #[test]
    fn the_sectors_functions_are_named_by_their_sectors() {
        assert_eq!(sector_function("Sector50039Init"), Some((50, 39)));
        assert_eq!(sector_function("Sector51039Enter"), Some((51, 39)));
        assert_eq!(sector_function("Sector0028Enter"), Some((0, 28)));
        assert_eq!(sector_function("Sector5001Init"), Some((5, 1)));
        // Not the engine's names: two digits only, another ending, another function.
        assert_eq!(sector_function("Sector28Enter"), None);
        assert_eq!(sector_function("Sector50039Exit"), None);
        assert_eq!(sector_function("QIS_OnEnter1"), None);
    }
}
