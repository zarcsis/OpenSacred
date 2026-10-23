//! The NPCs and objects that the scripts place: the `CreateNPC` and `CreateObj` records of the
//! hero's tree's start script, `StartCode.bin` (`netscript/<tree>.nsc`), of the `OnEnter`
//! functions of the quests that the start script enters (the novice in the chapel, quest 1's), and
//! of the sectors' `Init` and `Enter` functions (the villagers, the animals and the placed
//! monsters, docs/re/spawning.md), and the creatures that the `Enter` functions' `FillSector`
//! records make at random (`spawning`), each placed when its sector is in; an NPC plays the clip
//! of its model that the client plays standing, by the class of what it holds (`motions`), an
//! object (a barrel, a chest, a book stand, a signpost) stands, and a chest, a barrel or a book
//! stand takes its cell, which the creatures go round (`terrain::Heights::occupy`,
//! docs/re/movement.md, "The objects in the way"). An NPC
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
//! (`Item::angle`), and its model shows where its world position `0x20` does on the ground's
//! heights, where it has one, its cell staying its logical position (`Object`, `drawn_at`,
//! `model_position`).
//!
//! A position's third number is the layer the object lies in (**hypothesis**, docs/re/drawing.md):
//! the client draws every cell with the world's objects of layer 0, and a building's cells with
//! those of the building's current layer too, so an object or NPC on a building's floor shows
//! with its building's state (`buildings::Layered`). A walking NPC changes layers as the hero
//! does, onto a building's inside through its entrance and out at an exit (`0x5FCCB0`).
//!
//! An NPC holds its weapons and its shield as the client equips them (docs/re/equipment.md): each
//! item's model is attached by its socket bone, `Bone_weapon_02` in the left hand and
//! `Bone_weapon_01` in the right, to the NPC's hand, or its forearm for a shield. An archer that
//! goes to melee and back changes them (`rearm`, #396), with its values and its clips.
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
    AnimationClip, AnimationGraph, AnimationGraphHandle, AnimationNodeIndex, AnimationPlayer,
    AnimationTransitions, App, AssetId, AssetServer, Assets, ChildOf, Children, Commands,
    Component, DetectChangesMut, Entity, GlobalTransform, GltfAssetLabel, Handle, Has, Image, In,
    IntoScheduleConfigs, MeshMaterial3d, MessageWriter, Name, On, Plugin, Query, Remove, Res,
    ResMut, Resource, StandardMaterial, Time, Transform, Update, Vec2, Vec3, With, Without,
    WorldAssetRoot,
};
use bevy::world_serialization::{WorldAsset, WorldInstanceReady};
use opensacred_data::heroes::Worn;
use opensacred_data::netscript::{Body, Function, Record, Tree, Value};
use opensacred_data::tables::{Creature, Item, Weapon};
use opensacred_data::world::SIDE;
use opensacred_rules::{minds, ways};

use crate::frame::Step;
use crate::sectors::Streamer;
use crate::terrain::Heights;
use crate::terrain::Level;
use crate::{
    ai, arrows, buildings, crowd, daynight, drops, fight, hero, iso, models, motions, paths,
    shadows, spawning,
};

/// Record types of the scripts (`opensacred_formats::netscript::KEYWORDS`).
const CREATE_NPC: u16 = 1;
const CREATE_OBJ: u16 = 8;
const SET_NPC_STATE: u16 = 3;
/// The operand of a `CreateNPC` or a `SetNPCState` that names its dialogue, and that of a
/// `SetNPCState` that gives its number (`0x48BB40`).
const DIALOGUE: u8 = 0x09;
const DIALOGUE_NUMBER: u8 = 0x0A;
/// The operand of a `SetNPCState` that gives its creature's home, a global cell and its floor
/// (`0x461540`, `+0x2B9`).
const HOME: u8 = 0x4D;
/// The tags of `CreateNPC` that make a trader (`+0x200`): the smiths, the merchants, the wizards
/// and one with a dialogue stamped (`0x484326`…; docs/re/ai.md, "The roll").
const TRADES: [u8; 4] = [0x23, 0x24, 0x25, 0x64];
/// The tag of a trader whose dialogue `CreateNPC` stamps (`0x4843AF`).
const STAMPED: u8 = 0x64;
/// The tags of `CreateNPC` that set `+0x200` bit `0x200` (`0x482510`), which keeps a creature
/// out of its first update's flags (`opensacred_rules::minds::Mind::hold`).
const KEEPS: [u8; 4] = [0x1A, 0x70, 0x71, 0x72];
/// The operand of a `CreateNPC` that gives its creature's level, or makes it a boss
/// (`0x482510`, `fight::tagged_level`).
const LEVEL: u8 = 0x1F;
const TRIGGER_QUEST: u16 = 20;
const SET_UP_QUEST: u16 = 21;
const DEF_POS: u16 = 23;
const CREATE_TRIGGER: u16 = 48;
const TRIGGER_PATCH: u16 = 50;
const FILL_SECTOR: u16 = 51;
const SPAWN_VALUES: u16 = 100;
const WAFFENPOOL: u16 = 114;
const MAKE_GROUP: u16 = 115;
/// The tags of `CreateTrigger` that set and clear bit 0 of the trigger's state, its open bit
/// (`0x496520`).
const OPEN: u8 = 0x26;
const CLOSED: u8 = 0x27;
/// `CreateObj`'s operand that names its trigger.
const TRIGGER: u8 = 0x29;
/// `CreateObj`'s operand of the object's world position, `(x, y, layer)` (`0x485C10`).
const WORLD_POSITION: u8 = 0x20;
const START_POSITION: u16 = 45;
const NPC_GOTO: u16 = 72;

/// An NPC or an object to place.
#[derive(Clone, Default)]
struct Placement {
    /// The model, relative to the asset directory.
    model: String,
    /// The global cell, its logical position.
    cell: (u32, u32),
    /// Where an object's model is drawn, if its record gives it ([`drawn_at`]): world units on
    /// the ground, the game's x and z.
    drawn: Option<Vec2>,
    /// Degrees.
    heading: f32,
    /// A `CreateObj`'s object rather than an NPC.
    object: bool,
    /// The texture its item names (`+0x08`), relative to the asset directory.
    texture: Option<String>,
    /// The position's third number, the layer.
    layer: i32,
    /// The items an NPC holds in its hands: their models, and the items themselves.
    equipment: Vec<Held>,
    holding: Vec<InHand>,
    /// A fill's boss or champion, or a script's boss (`fight::Rank`).
    rank: fight::Rank,
    /// An NPC's `CreateNPC` level tag ([`LEVEL`]), its last, or the last `SetNPCState`'s that
    /// names it ([`set_states`]), 0 for none, by which it takes its level and may be a boss
    /// (`fight::tagged_level`, `fight::Rank::of_tag`).
    tag: u32,
    /// Whether it casts a shadow (`shadows::Casts`).
    casts: bool,
    /// Whether it stops the NPCs' steps into its cell, an object of category 4 (`0x4F7E10`
    /// refuses its cell to a creature of id above 16, not to the hero; `crowd`).
    blocks: bool,
    /// Whether it is a door, an object of category 10, which the townsfolk go in and out by
    /// (`0x428130`; `ai`), and the trigger it names (`CreateObj`'s `0x29`), whose cells it opens
    /// and closes (#372).
    door: bool,
    trigger: Option<String>,
    /// An NPC's creature, its `Creature.pak` record's flags and the flags word its record's tags
    /// make (`crowd::flags_word`): how it stands in the others' way.
    creature: (u32, u8, u32),
    /// An NPC's side (`+0x1F0`), by which it is hostile to the hero or not
    /// (`opensacred_rules::minds::hostile_to_heroes`; #580).
    side: u8,
    /// An NPC's walk and run speeds in world units a second, before the game's speed, and its
    /// `Creature.pak` class (`ai::Mind`).
    speed: [f32; 2],
    class: u8,
    /// An NPC's `Creature.pak` values of the fight, from which its health and its strikes
    /// (`fight`), the class of what it holds (`held_class`), whether that is a ranged weapon
    /// (`ranged`), and its type's size for the arrows' hits (`items.pak +0x14`, `arrows::Size`).
    base: fight::Base,
    held: u8,
    ranged: bool,
    size: f32,
    /// Whether a sector's `Enter` function made it, and whether `CreateNPC` did rather than a
    /// fill; its group, if it is in one; whether it has a dialogue, a `CreateNPC` with one
    /// (the operand `0x09`; **hypothesis**: the one `0x483AFA` stamps; `ai::Mind`).
    made: (bool, bool),
    /// Whether the start made it, its script or a quest it entered rather than a sector's
    /// function: its creature takes its level as the game starts ([`place`]).
    started: bool,
    dialogue: bool,
    /// Whether it is a trader, by its tags (`opensacred_rules::minds::Mind::trader`), and whether
    /// its tags keep it out of its first update's flags ([`KEEPS`]).
    trader: bool,
    kept: bool,
    group: Option<u32>,
    /// The cell whose centre an NPC goes home to, where a `SetNPCState` gives one, else where it
    /// was made (`0x4D`, `+0x2B9`; [`set_states`]).
    home: Option<(u32, u32)>,
    /// Where an NPC walks from the start (`NPC_Goto`), the global cell, and its speed in world
    /// units a second, before the game's speed: its walk speed, or its run speed where its walk
    /// is a run.
    walk: Option<((u32, u32), f32)>,
    /// Whether the object maker alone made it (`0x5FBA40`), a crab or a fish of the waters, which
    /// no level's setter raises: its skills its two slots' at level 1, as its stats are made
    /// (`0x586C00`), and its most health the stats' 100 (seen in the original, #575).
    plain: bool,
    /// The clips an NPC plays.
    clips: ClipNames,
}

/// The clips an NPC plays, by name (`motions`): standing, walking where a script sends it,
/// running where it flees, and its fight stance where its model has one
/// (`motions::Slots::stance`).
#[derive(Clone, Debug, Default)]
struct ClipNames {
    stand: Option<String>,
    walk: Option<String>,
    run: Option<String>,
    stance: Option<String>,
    talk: Option<String>,
    /// The jugglers' clips of the slots 180 to 183 (`*_FUN_IDLE_*`), by which they play their
    /// instruments.
    fun: [Option<String>; 4],
    /// Its attack's clips (`motions::Slots::attacks`).
    attacks: Vec<String>,
    /// Its dying clips, the slots 173 and 174 (`*_DYING_A`, `*_DYING_B`; **hypothesis** for the
    /// dying action 6's choice).
    dying: Vec<String>,
}

impl ClipNames {
    /// The clips of a model of `slots` that holds what is of `class` (`motions::class`).
    fn of(slots: &motions::Slots, class: u8) -> Self {
        let clip = |action| slots.clip(action, class).map(|(name, _)| name.to_owned());
        Self {
            stand: clip(motions::Action::Stand),
            walk: clip(motions::Action::Walk),
            run: clip(motions::Action::Run),
            stance: slots.stance(class).map(str::to_owned),
            attacks: slots.attacks(class),
            dying: [173, 174]
                .iter()
                .filter_map(|&slot| slots.slot(slot).map(str::to_owned))
                .collect(),
            talk: slots.talk().map(str::to_owned),
            fun: [180, 181, 182, 183].map(|slot| slots.slot(slot).map(str::to_owned)),
        }
    }
}

/// The instruments a juggler takes (`0x4F29E2`): the flute, the bell, the harp and the horn, their
/// `items.pak` slots and the slots of the clips that play them.
const INSTRUMENTS: [(u32, u16); 4] = [(5108, 183), (5109, 180), (5110, 182), (5111, 181)];
/// The first of the jugglers' clips' slots; the bell's, which one without an instrument plays.
const FUN: u16 = 180;

/// The item categories of `items.pak +0x2E` that go in a hand (docs/02 §6.1).
const WEAPON: u8 = 5;
const SHIELD: u8 = 13;
/// The category of the chests, the barrels, the vases, the coffins, the cabinets, the book stands
/// and the shrines, which the client makes `cChest3D`s, whose draw draws their shadows
/// (`0x451300`, `0x5C97B0`), and which take their cells: a creature does not step into a cell
/// that holds one (`0x4F7E10`); the signposts are of 15, `cSpecial3D`, which casts none and
/// takes no cell.
const CHEST: u8 = 4;
/// The category of the doors (`items.pak` 5650 to 5766, `0x428130`).
const DOOR: u8 = 10;
/// The flag of a creature that casts no shadow (`Creature.pak +0x06`, `0x426D30`).
const NO_SHADOW: u8 = 0x10;
/// The weapon kinds of `Weapon.pak +0x1E` held in the left hand: bows and crossbows.
const BOW: u8 = 7;
const CROSSBOW: u8 = 12;
/// The weapon kinds of the ranged weapons, with which a creature fights at range (`0x427ED0`;
/// docs/re/fight.md, "Who fights"): the bows, the crossbows, 13 and the muskets.
const RANGED: [u8; 4] = [BOW, CROSSBOW, 13, 14];
/// The arrows' type, `ARROW_LONG.GRN`, of the archers' shots (`0x4EB700`).
const ARROW: u32 = 3070;

/// A hand of an NPC's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hand {
    Left,
    Right,
}

/// An item an NPC or the hero holds.
#[derive(Clone, Debug)]
pub struct Held {
    /// The model, relative to the asset directory.
    model: String,
    /// The texture its item names (`+0x08`).
    texture: Option<String>,
    hand: Hand,
    /// A shield, held on the forearm rather than in the hand.
    shield: bool,
}

/// An item in an NPC's hand, whose values it fights with: its type, its category, its weapon's
/// kind, and the hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InHand {
    item: u32,
    category: u8,
    kind: Option<u8>,
    hand: Hand,
}

impl InHand {
    /// The items of `items`, each of its type, category and kind, that go in a hand (`hands`).
    fn of(items: impl IntoIterator<Item = (u32, u8, Option<u8>)>) -> Vec<Self> {
        let items: Vec<(u32, u8, Option<u8>)> = items.into_iter().collect();
        let categories: Vec<(u8, Option<u8>)> = items.iter().map(|&(_, c, k)| (c, k)).collect();
        items
            .iter()
            .zip(hands(&categories))
            .filter_map(|(&(item, category, kind), hand)| {
                Some(Self {
                    item,
                    category,
                    kind,
                    hand: hand?,
                })
            })
            .collect()
    }
}

/// The class of what an NPC holds, its items in their hands (`motions::class`; [`held_class`]).
fn class_of(holding: &[InHand]) -> u8 {
    let in_hand = |hand| holding.iter().find(|held| held.hand == hand);
    motions::class(
        in_hand(Hand::Left).map(|held| (held.category == SHIELD, held.kind.unwrap_or(0))),
        in_hand(Hand::Right).map(|held| held.kind.unwrap_or(0)),
    )
}

/// Whether an NPC holds a ranged weapon, its items in their hands ([`ranged`]).
fn ranged_of(holding: &[InHand]) -> bool {
    holding
        .iter()
        .any(|held| held.category == WEAPON && held.kind.is_some_and(|kind| RANGED.contains(&kind)))
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

/// Whether an NPC holds a ranged weapon in a hand (`hands`): a weapon of one of the kinds
/// [`RANGED`] (`0x54A2A0`).
fn ranged(items: &[(u8, Option<u8>)]) -> bool {
    items
        .iter()
        .zip(hands(items))
        .any(|(&(category, kind), hand)| {
            hand.is_some() && category == WEAPON && kind.is_some_and(|kind| RANGED.contains(&kind))
        })
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

/// What a creature type is made of for its template ([`template`]): its item's model, texture
/// and size (`items.pak`, by the type's slot), and its `Creature.pak` speeds, class, fight values
/// and flags, and whether it casts a shadow.
#[derive(Clone)]
struct Kind {
    model: String,
    texture: Option<String>,
    size: f32,
    speed: [f32; 2],
    class: u8,
    base: fight::Base,
    flags: u8,
    casts: bool,
}

/// What a fill's creature of type `creature`, of `kind`, whose model has the clips `slots`, is
/// placed as: its model standing with its hands empty, its shadow, and its flags word, to which a
/// hostile entry adds bit 0.
fn template(creature: u32, kind: &Kind, slots: &motions::Slots) -> Placement {
    Placement {
        model: kind.model.clone(),
        cell: (0, 0),
        drawn: None,
        home: None,
        walk: None,
        speed: kind.speed,
        class: kind.class,
        base: kind.base,
        held: held_class(&[]),
        ranged: false,
        size: kind.size,
        made: (true, false),
        started: false,
        dialogue: false,
        trader: false,
        kept: false,
        group: None,
        heading: 0.0,
        object: false,
        casts: kind.casts,
        blocks: false,
        door: false,
        trigger: None,
        creature: (creature, kind.flags, 0),
        side: kind.class,
        texture: kind.texture.clone(),
        layer: 0,
        equipment: Vec::new(),
        holding: Vec::new(),
        rank: fight::Rank::Common,
        tag: 0,
        clips: ClipNames::of(slots, held_class(&[])),
        plain: false,
    }
}

#[derive(Resource)]
pub struct Npcs {
    placements: Vec<Placement>,
    by_sector: HashMap<(u32, u32), Vec<usize>>,
    /// The cells of the closed triggers ([`closed_cells`]), by sector, and by trigger; the
    /// triggers that a door has opened since (#372), whose cells a sector that comes in leaves
    /// free, and those that a hero has turned, whose doors no townsperson closes.
    closed: HashMap<(u32, u32), Vec<(String, Spot)>>,
    triggers: HashMap<String, Vec<Spot>>,
    opened: HashSet<String>,
    marked: HashSet<String>,
    /// The sectors' values, which their creatures' levels take, and their fills ([`sectors`]),
    /// and what each type they make is placed as.
    values: HashMap<(u32, u32), spawning::SpawnValues>,
    fills: HashMap<(u32, u32), Vec<spawning::Fill>>,
    templates: HashMap<u32, Placement>,
    /// What each creature type with a model is made of, from which a template of a type that no
    /// fill makes is made when a command spawns one ([`Npcs::template`]).
    types: HashMap<u32, Kind>,
    /// Each creature type's `Creature.pak` class, which picks a fill's factor.
    classes: HashMap<u32, u8>,
    factors: spawning::Factors,
    /// The balance's values of the creatures' fight, and the templates of the items' values.
    fight: fight::Rules,
    items: fight::Templates,
    /// The weapon pools, the sectors' `MakeGroup` rules, what each pool's item and each weapon of
    /// the archers' switch is in a hand, and the templates' models' clips by slot, which give the
    /// fills' creatures their items and the clips of what they hold ([`gear_of`], [`rearm`]).
    pools: HashMap<i32, Vec<u32>>,
    rules: HashMap<(u32, u32), Vec<Rule>>,
    gear: HashMap<u32, Gear>,
    slots: HashMap<String, motions::Slots>,
    /// The instruments' models and textures, of [`INSTRUMENTS`].
    instruments: Vec<Option<(String, Option<String>)>>,
    /// The arrows' model, texture and size ([`ARROW`]).
    arrow: Option<(String, Option<String>, f32)>,
    /// The fractions of the attack's clips at which they hit, by the clips' names upper-cased
    /// (`MotionDesc +0xA0`, `+0xA4`: `floats_62[1]` and `[2]` of `tables/motions.ron`).
    hits: HashMap<String, [f32; 2]>,
    spawned: HashSet<(u32, u32)>,
    /// What became of the creatures that went with their sectors ([`Kept`]).
    kept: Kept,
    /// The sectors come in anew whose waters' crabs and fish wait for the sectors about them
    /// ([`swimmers`]).
    waters: Vec<(u32, u32)>,
}

/// Where a creature came from, which decides what becomes of it when its sector goes, as the
/// original's sectors' functions keep their creatures (#329, docs/re/spawning.md, "The sectors'
/// functions"). A sector's `Enter` makes its villagers and animals again at every coming in, and
/// they carry none.
#[derive(Component, Clone)]
pub struct Placed {
    /// The sector that placed it.
    at: (u32, u32),
    source: Source,
}

#[derive(Clone)]
enum Source {
    /// A `CreateNPC` of the start script or of a sector's `Init`, the placement of that index:
    /// placed once, it keeps where it stands and its health, and a killed one does not come back.
    Kept(usize),
    /// A creature that a sector's fill made: the sector keeps it while it lives, and puts it back
    /// at its next coming in rather than filling anew (`0x4954C0`, `0x4939F0`).
    Filled(Box<Placement>),
}

/// Where a creature that went with its sector stood, on which floor, and its health.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Stood {
    cell: (u32, u32),
    layer: i32,
    health: fight::Health,
}

/// What became of the placed creatures ([`Placed`]) that went with their sectors.
#[derive(Default)]
struct Kept {
    /// The kept placements that went, by index: where each stood, or `None` once killed.
    went: HashMap<usize, Option<Stood>>,
    /// The kept placements spawned, living somewhere: not placed again while they are.
    out: HashSet<usize>,
    /// The living creatures of each sector's fills that went with it.
    filled: HashMap<(u32, u32), Vec<(Placement, Stood)>>,
    /// How many creatures of each sector's fills are spawned, living somewhere.
    filled_out: HashMap<(u32, u32), usize>,
}

/// What a sector that comes in does with a kept placement.
#[derive(Debug, PartialEq)]
enum Again {
    /// Places it as its record has it, the first time.
    Anew,
    /// Places it where it stood, with its health.
    AtStood(Stood),
    /// Places nothing: it was killed, or it stands somewhere still.
    Not,
}

impl Kept {
    /// What becomes of kept placement `index` as its sector comes in.
    fn again(&self, index: usize) -> Again {
        if self.out.contains(&index) {
            return Again::Not;
        }
        match self.went.get(&index) {
            None => Again::Anew,
            Some(Some(stood)) => Again::AtStood(*stood),
            Some(None) => Again::Not,
        }
    }

    /// The fills' living creatures that sector `at` puts back, and whether it fills anew: only
    /// when none of them lives, neither kept nor somewhere still.
    fn back(&mut self, at: (u32, u32)) -> (Vec<(Placement, Stood)>, bool) {
        let back = self.filled.remove(&at).unwrap_or_default();
        let anew = back.is_empty() && self.filled_out.get(&at).copied().unwrap_or(0) == 0;
        (back, anew)
    }

    /// Notes that a placed creature is spawned.
    fn spawned(&mut self, placed: &Placed) {
        match placed.source {
            Source::Kept(index) => {
                self.out.insert(index);
            }
            Source::Filled(_) => *self.filled_out.entry(placed.at).or_default() += 1,
        }
    }

    /// Notes that a placed creature goes, `stood` where it stood if it lives.
    fn gone(&mut self, placed: &Placed, stood: Option<Stood>) {
        match &placed.source {
            Source::Kept(index) => {
                self.out.remove(index);
                self.went.insert(*index, stood);
            }
            Source::Filled(placement) => {
                if let Some(count) = self.filled_out.get_mut(&placed.at) {
                    *count = count.saturating_sub(1);
                }
                if let Some(stood) = stood {
                    self.filled
                        .entry(placed.at)
                        .or_default()
                        .push(((**placement).clone(), stood));
                }
            }
        }
    }
}

/// A global cell and a floor (or a layer), as the scripts give a position: `(x, y, floor)`.
type Spot = (i32, i32, i32);

/// An object that a script places (a `CreateObj` record), and its logical position: the centre
/// of its `0x04` cell on its level, which the client keeps as the object's position while it
/// draws the model at its `0x20` (`drawn_at`, `0x485C10`, docs/02 §7.3). The cell that a chest
/// takes in the creatures' way, a door's distances, the doors that the creatures choose and the
/// model's place among the sprites go by it, as the client chains the object into its cell's
/// list (`0x5FCCB0`) that the row walk draws (`0x62AE90`); the model and its shadow by the
/// entity's `Transform`.
#[derive(Component)]
pub struct Object {
    pub at: Vec3,
}

/// The texture a placed model takes in place of its own: its item's (`items.pak +0x08`).
#[derive(Component)]
pub struct Retexture(Handle<Image>);

impl Retexture {
    /// The texture `texture`, relative to the asset directory, repeated ([`load_texture`]).
    pub fn load(assets: &AssetServer, texture: &str) -> Self {
        Self(load_texture(assets, texture))
    }
}

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
    /// Whether both its hands are empty, and the instrument it took, of [`INSTRUMENTS`].
    hands_empty: bool,
    instrument: Option<usize>,
    /// Its hits' share of a strike: 0.5 with two weapons, else 1 (`0x4EA3F0`).
    share: f32,
    /// The factor of its walk and run speeds and of their clips' rate, its pace's
    /// (`opensacred_rules::fight::Combat::pace_factor`, #599).
    pace: f32,
}

/// An NPC's animation graph: its idle clip, and its walk, run and stance clips if it has them.
#[derive(Clone)]
struct Clips {
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    walk: Option<AnimationNodeIndex>,
    run: Option<AnimationNodeIndex>,
    stance: Option<AnimationNodeIndex>,
    talk: Option<AnimationNodeIndex>,
    fun: [Option<AnimationNodeIndex>; 4],
    /// The attack's clips, each with its clip and the fractions at which it hits.
    attacks: Vec<(AnimationNodeIndex, Handle<AnimationClip>, [f32; 2])>,
    dying: Vec<AnimationNodeIndex>,
}

impl Clips {
    /// The clip of a walk: its run clip where it runs and has one, else its walk clip.
    fn moving(&self, walk: &Walk) -> Option<AnimationNodeIndex> {
        if !walk.moved {
            None
        } else if walk.runs {
            self.run.or(self.walk)
        } else {
            self.walk
        }
    }
}

/// An NPC walking where a script sent it (`NPC_Goto`, `0x49E210`, which orders the creature to
/// walk there): the point, the level it walks on, and its walk speed in world units a second,
/// before the game's speed.
#[derive(Component)]
pub struct Walk {
    target: Vec3,
    level: Level,
    speed: f32,
    /// Whether it runs, playing its run clip.
    runs: bool,
    /// Whether it stands on a stair it has climbed already or arrived on.
    climbed: bool,
    /// Whether it has stepped yet: its walk clip plays from then on.
    moved: bool,
    /// How long it still stands where a walk that could not start left it, in seconds of the
    /// game: the walk ends then, so that no behaviour orders it again at every frame.
    balked: f32,
    /// How near its target it stops, in world units.
    within: f32,
    /// Whether it comes on at the hero in a fight (`ai::pursue`), which follows the hero as it
    /// moves, rather than a flight, a step back or a walk about (#387).
    comes_on: bool,
}

/// How long a creature whose walk could not start stands before its walk ends, in seconds.
const BALKS: f32 = 1.0;

impl Walk {
    /// A walk to `target` from `level` at `speed`, in world units a second before the game's
    /// speed.
    pub fn to(target: Vec3, level: Level, speed: f32) -> Self {
        Self {
            target,
            level,
            speed,
            runs: false,
            climbed: false,
            moved: false,
            balked: 0.0,
            within: hero::ARRIVED,
            comes_on: false,
        }
    }

    /// The walk, stopping within `within` of its target.
    pub fn within(self, within: f32) -> Self {
        Self { within, ..self }
    }

    /// Its target.
    pub fn target(&self) -> Vec3 {
        self.target
    }

    /// Whether it runs.
    pub fn runs(&self) -> bool {
        self.runs
    }

    /// The walk, one that comes on at the hero in a fight (#387, `ai::pursue`).
    pub fn coming_on(self) -> Self {
        Self {
            comes_on: true,
            ..self
        }
    }

    /// Whether it comes on at the hero in a fight.
    pub fn comes_on(&self) -> bool {
        self.comes_on
    }

    /// A run to `target` from `level` at `speed`.
    pub fn run(target: Vec3, level: Level, speed: f32) -> Self {
        Self {
            runs: true,
            ..Self::to(target, level, speed)
        }
    }
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

/// Where a `CreateObj`'s model is drawn: its world position `0x20`, `(x, y)` in world units on the
/// ground, the game's x and z, if it has one. The client keeps the object's position at its
/// `0x04` cell's centre (`0x487130`) and sets the floats `+0xCC…+0xD4` from `0x20` with the
/// object's flag `0x800000` (`0x485C10`): where the model is drawn, by which 1,114 of the 1,125
/// doors' leaves stand in their doorways, every gate centred on its model's origin among them,
/// against 997 at their cells' centres, and the others on raised doorways too, seen at height 0
/// (`model_position`, docs/02 §7.3). The third number is no height: it repeats the cell's layer
/// in every record of the Seraphim's tree.
fn drawn_at(ops: &[opensacred_data::netscript::Operand]) -> Option<Vec2> {
    ops.iter().find_map(|o| match (o.tag, &o.value) {
        (WORLD_POSITION, Value::I32x3([x, y, _])) => Some(Vec2::new(*x as f32, *y as f32)),
        _ => None,
    })
}

/// Where the model of a placement whose logical position is `at` stands: at `at` without a drawn
/// position; with one, where the screen shows its drawn position `drawn` at height 0, taken along
/// the line of sight to `at`'s height, as the client draws it: `0x485C10` moves `0x20` by its
/// object's height `h` at its cell's centre on its level (`0x449950`), `1.92 · h` world units
/// along x and y for a unit of the models' height, into the floats `+0xCC…+0xD4`, the model's
/// position (`0x44B230`), never reading the ground's height at `0x20` (docs/02 §7.3). So a raised
/// doorway's `0x20`, on the ground up the screen from it by its height, shows its leaf in it.
fn model_position(at: Vec3, drawn: Option<Vec2>) -> Vec3 {
    drawn.map_or(at, |drawn| {
        iso::at_height(Vec3::new(drawn.x, 0.0, drawn.y), at.y)
    })
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

/// The records that place the world's NPCs and objects: the start script's, with the `OnSetUp`
/// function of each quest it sets up after its `SetUpQuest`, then those of the
/// `OnEnter` function of each quest it enters, which the client runs as it enters the quest
/// (`QIS_OnEnter<n>`, `0x46C160`), then the sectors' `Init` and `Enter` functions', which the
/// engine runs as their sectors come near (docs/re/spawning.md); each record's NPC or object is
/// placed as the sector of its cell comes in.
fn start_records(tree: &Tree) -> Vec<&Record> {
    // The start script, each quest's `OnSetUp` after the `SetUpQuest` that first sets it up, as
    // the client runs it there (`0x48F7F0`, `0x46DB20`; #624): the quests' givers among them.
    let mut records: Vec<&Record> = Vec::new();
    let mut set_up = HashSet::new();
    for record in &tree.start {
        records.push(record);
        if record.kind == SET_UP_QUEST
            && let Some(quest) = quest_of(record)
            && set_up.insert(quest)
            && let Some(on_set_up) = function(tree, &format!("QIS_OnSetUp{quest}"))
        {
            records.extend(&on_set_up.records);
        }
    }
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

/// What the sectors' `Enter` functions hold for their creatures ([`sectors`]).
#[derive(Debug, Default)]
struct Sectors {
    /// Each sector's `SpawnValues`, which its `Init` repeats in every tree.
    values: HashMap<(u32, u32), spawning::SpawnValues>,
    /// The `FillSector` records of the sectors that have some.
    fills: HashMap<(u32, u32), Vec<spawning::Fill>>,
}

/// The `SpawnValues` and the `FillSector` records of the sectors' `Enter` functions, by sector
/// (docs/re/spawning.md, "The sector's level", "`FillSector`"): the values of every sector that
/// has them, with fills or without, which the levels of the creatures of its cells take.
fn sectors(tree: &Tree) -> Sectors {
    let mut out = Sectors::default();
    for function in tree.functions.iter().filter(|f| f.name.ends_with("Enter")) {
        let Some(sector) = sector_function(&function.name) else {
            continue;
        };
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
                        let values = spawning::SpawnValues {
                            density,
                            lowest,
                            highest,
                        };
                        out.values.insert(sector, values);
                    }
                }
                FILL_SECTOR => fills.extend(fill_of(ops)),
                _ => {}
            }
        }
        if !fills.is_empty() {
            out.fills.insert(sector, fills);
        }
    }
    out
}

/// An item of the weapon pools as an NPC holds it: its category, its weapon's kind, its model and
/// its texture.
#[derive(Clone, Debug)]
struct Gear {
    category: u8,
    kind: Option<u8>,
    model: Option<String>,
    texture: Option<String>,
    /// Whether its values count in its holder's: not where its `items.pak +0x03` has bit 1
    /// (`0x426550`, bit 25 of the record's first word), as the heroes' hair, the Gladiator's
    /// belt and the Mage's cowl have, which the stats' recomputation `0x5796A0` passes over (#490).
    counts: bool,
}

/// A `MakeGroup` rule (`0x4AB940`): the creature types it equips, and its three items' pools and
/// chances in percent (`87`, `88`, `89`; a pool of −1 or a chance of 0 gives nothing).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Rule {
    types: Vec<u32>,
    items: [(i32, u32); 3],
}

/// The scripts' weapon pools (`Waffenpool`, type 114, `0x475F1A`): each record adds its last
/// `02` item to its `0B` pool, kept in order, a repeated item weighing more.
fn pools(records: &[&Record]) -> HashMap<i32, Vec<u32>> {
    let mut pools: HashMap<i32, Vec<u32>> = HashMap::new();
    for ops in records
        .iter()
        .filter(|r| r.kind == WAFFENPOOL)
        .filter_map(|r| match &r.body {
            Body::Operands(ops) => Some(ops),
            _ => None,
        })
    {
        let pool = ops.iter().find_map(|o| match (o.tag, &o.value) {
            (0x0B, Value::I32(v)) => Some(*v),
            _ => None,
        });
        let item = ops
            .iter()
            .filter_map(|o| match (o.tag, &o.value) {
                (0x02, Value::I32(v)) => u32::try_from(*v).ok(),
                _ => None,
            })
            .next_back();
        if let (Some(pool), Some(item)) = (pool, item)
            && (0..255).contains(&pool)
            && item > 0
        {
            pools.entry(pool).or_default().push(item);
        }
    }
    pools
}

/// The `MakeGroup` rules of the sectors' `Enter` functions, by sector, in order.
fn rules(tree: &Tree) -> HashMap<(u32, u32), Vec<Rule>> {
    let mut out: HashMap<(u32, u32), Vec<Rule>> = HashMap::new();
    for function in tree.functions.iter().filter(|f| f.name.ends_with("Enter")) {
        let Some(sector) = sector_function(&function.name) else {
            continue;
        };
        for ops in function
            .records
            .iter()
            .filter(|r| r.kind == MAKE_GROUP)
            .filter_map(|r| match &r.body {
                Body::Operands(ops) => Some(ops),
                _ => None,
            })
        {
            let types = ops
                .iter()
                .filter_map(|o| match (o.tag, &o.value) {
                    (0x02, Value::I32(v)) => u32::try_from(*v).ok(),
                    _ => None,
                })
                .take(20)
                .collect();
            let mut items = [(-1, 0), (0, 0), (-1, 0)];
            for o in ops {
                if let (0x87..=0x89, Value::I32x2([pool, chance])) = (o.tag, &o.value) {
                    items[usize::from(o.tag - 0x87)] = (*pool, u32::try_from(*chance).unwrap_or(0));
                }
            }
            out.entry(sector).or_default().push(Rule { types, items });
        }
    }
    out
}

/// The items 1800 to 1804, which a rule's first item becomes 1723, 1720, 1759, 1729 and 1752 for
/// most types (`0x47F8E0`).
const SWAPPED: [u32; 5] = [1723, 1720, 1759, 1729, 1752];
/// The weapon kinds after which a creature takes no second item: the two-handed, the staves, the
/// bows and the crossbows (**hypothesis** for the names).
const ONE_ITEM: [u8; 5] = [2, 3, 4, 7, 12];

/// The items a creature of `creature` takes by the first of the sector's rules that names it
/// (`0x47F8E0`): each of the first two by its chance from its pool, at random among the pool's
/// items, the first swapped ([`SWAPPED`]), the second only after a weapon of a kind that leaves
/// a hand free; the third, a worn piece, is left out, the game drawing no worn models.
fn gear_of<'a>(
    rules: &[Rule],
    creature: u32,
    pools: &HashMap<i32, Vec<u32>>,
    gear: &'a HashMap<u32, Gear>,
    rng: &mut spawning::Rng,
) -> Vec<(u32, &'a Gear)> {
    let Some(rule) = rules.iter().find(|r| r.types.contains(&creature)) else {
        return Vec::new();
    };
    let mut roll = |(pool, chance): (i32, u32)| {
        let made = rng.below(100) < chance;
        let list = pools.get(&pool).filter(|list| !list.is_empty())?;
        let index = usize::try_from(rng.below(u32::try_from(list.len()).ok()?)).ok()?;
        made.then(|| list[index])
    };
    let first = roll(rule.items[0]).map(|item| match item {
        1800..=1804 => SWAPPED[usize::try_from(item - 1800).unwrap_or(0)],
        other => other,
    });
    let second = roll(rule.items[1]);
    let first = first.and_then(|item| Some((item, gear.get(&item)?)));
    let second = second
        .filter(|_| {
            first.is_some_and(|(_, g)| {
                g.category == WEAPON && !g.kind.is_some_and(|k| ONE_ITEM.contains(&k))
            })
        })
        .and_then(|item| Some((item, gear.get(&item)?)));
    first.into_iter().chain(second).collect()
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

/// The fractions of the attack's clips at which they hit (`0x4EA1F0`, docs/re/fight.md), none
/// where `tables/motions.ron` cannot be read.
pub fn hit_fractions(assets: &Path) -> HashMap<String, [f32; 2]> {
    let path = assets.join("tables").join(opensacred_data::tables::MOTIONS);
    let motions: Vec<opensacred_data::tables::Motion> = fs::read_to_string(&path)
        .ok()
        .and_then(|text| ron::from_str(&text).ok())
        .unwrap_or_default();
    motions
        .iter()
        .filter(|m| m.floats_62[1] > 0.0 || m.floats_62[2] > 0.0)
        .map(|m| {
            let name = m.name.to_ascii_uppercase();
            let name = name.strip_suffix(".GRN").unwrap_or(&name).to_owned();
            (name, [m.floats_62[1], m.floats_62[2]])
        })
        .collect()
}

/// The floats of each motion from `MotionDesc +0xA0` (`floats_62[1..]` of `tables/motions.ron`),
/// whose leading ones above 0 are the hit points of an art's clip (#509,
/// `opensacred_rules::arts::hit_points`), by the motion's name as [`hit_fractions`] has it; none
/// where the table cannot be read.
pub fn hit_points(assets: &Path) -> HashMap<String, Vec<f32>> {
    let path = assets.join("tables").join(opensacred_data::tables::MOTIONS);
    let motions: Vec<opensacred_data::tables::Motion> = fs::read_to_string(&path)
        .ok()
        .and_then(|text| ron::from_str(&text).ok())
        .unwrap_or_default();
    motions
        .iter()
        .map(|m| {
            let name = m.name.to_ascii_uppercase();
            let name = name.strip_suffix(".GRN").unwrap_or(&name).to_owned();
            (name, m.floats_62[1..].to_vec())
        })
        .collect()
}

/// The balance's factors of the fills' density (`SpawnAnimalsSP`, `SpawnEnemysSP`), 0.8 and 1.2
/// where `tables/balance.ron` lacks them.
fn factors(balance: Option<&opensacred_data::tables::Balance>) -> spawning::Factors {
    let factor = |name, default| balance.and_then(|b| b.f32(name)).unwrap_or(default);
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

/// A `CreateNPC`'s level tag ([`LEVEL`]), the last of its operands', as `CreateNPC` keeps the
/// last (`0x482510`); 0 for none, or for a variable or a number below 0 in its place, which no
/// `CreateNPC` of the scripts carries.
fn level_tag(ops: &[opensacred_data::netscript::Operand]) -> u32 {
    ops.iter()
        .rev()
        .find_map(|o| match (o.tag, &o.value) {
            (LEVEL, Value::I32(v)) => Some(u32::try_from(*v).unwrap_or(0)),
            (LEVEL, _) => Some(0),
            _ => None,
        })
        .unwrap_or(0)
}

/// What a `SetNPCState` record changes in the creature its `01` names, of what the game keeps of a
/// creature (`0x48BB40`, which reads the record's tags, and `0x461540`, which applies them;
/// docs/re/netscript.md, "`SetNPCState`").
#[derive(Clone, Debug, Default, PartialEq)]
struct State {
    /// Its level tag ([`LEVEL`]), the last; 0 for none.
    level: u32,
    /// Whether it gives the creature a dialogue or takes its dialogue away: the last of a
    /// dialogue's name (`0x09`) found in the dialogues' table at an index above 0, or not, and
    /// of a dialogue's number (`0x0A`), which always takes it away, the table's entries keeping
    /// none (`0x476A05`).
    dialogue: Option<bool>,
    /// The flags word that its operand-less tags make (`ways::state_word`), 0 for none.
    word: u32,
    /// The last of `0x08` (hostile) and `0x0E` (peaceful).
    hostile: Option<bool>,
    /// Whether one of the tags `0x23`, `0x24`, `0x25` or `0x64` makes it a trader (`+0x200`
    /// bits `0x1000`, `0x2000`, `0x4000` or `0x4000000`), and whether `0x64` stamps its
    /// trader's dialogue.
    trader: bool,
    stamped: bool,
    /// Its home (`0x4D`, `+0x2B9`): the cell whose centre it goes home to.
    home: Option<(u32, u32)>,
}

impl State {
    /// The state of a `SetNPCState` of operands `ops`, its dialogues' names looked up in the
    /// dialogues' table `dialogues` as it stands when the record runs.
    fn of(ops: &[opensacred_data::netscript::Operand], dialogues: &[String]) -> Self {
        let markers = ops
            .iter()
            .filter(|o| matches!(o.value, Value::Marker))
            .map(|o| o.tag);
        let mut state = Self {
            level: level_tag(ops),
            word: ways::state_word(markers),
            ..Self::default()
        };
        for op in ops {
            match (op.tag, &op.value) {
                (DIALOGUE, Value::Str(name)) => {
                    let index = dialogues.iter().position(|d| d.eq_ignore_ascii_case(name));
                    state.dialogue = Some(index.is_some_and(|i| i > 0));
                }
                (DIALOGUE_NUMBER, _) => state.dialogue = Some(false),
                (0x08, Value::Marker) => state.hostile = Some(true),
                (0x0E, Value::Marker) => state.hostile = Some(false),
                (tag, Value::Marker) if TRADES.contains(&tag) => {
                    state.trader = true;
                    state.stamped |= tag == STAMPED;
                }
                (HOME, Value::I32x3([x, y, _])) => {
                    if let (Ok(x), Ok(y)) = (u32::try_from(*x), u32::try_from(*y)) {
                        state.home = Some((x, y));
                    }
                }
                _ => {}
            }
        }
        state
    }

    /// Applies it to `placement`, in the order of `0x461540`: the flags word, which takes a
    /// trader's trade away with the mode `0x800`; the home; the side by the hostility; the
    /// dialogue; the level, 1000 a boss; the trades, `0x64`'s dialogue last.
    fn apply(&self, placement: &mut Placement) {
        let (creature, _, word) = &mut placement.creature;
        let mut stated = self.word;
        if stated & 0x800 != 0 && placement.trader {
            stated &= !0x800;
            placement.trader = false;
        }
        *word = ways::stated_word(*word, stated);
        if self.home.is_some() {
            placement.home = self.home;
        }
        if let Some(hostile) = self.hostile {
            placement.side = minds::stated_side(placement.side, *creature, stated, hostile);
        }
        if let Some(dialogue) = self.dialogue {
            placement.dialogue = dialogue;
        }
        if self.level > 0 {
            placement.tag = self.level;
            if self.level == fight::BOSS_TAG {
                placement.rank = fight::Rank::Boss;
            }
        }
        placement.trader |= self.trader;
        placement.dialogue |= self.stamped;
    }
}

/// Gives the creatures that `records`' `SetNPCState`s name what those change of them
/// ([`State`]; #541, #578): each placement of a `CreateNPC` before the `SetNPCState` that names
/// it, `made` the placement of each record; the stables' horses their levels, after their
/// `CreateNPC`s' rule, and the start's captain of 52,39 its dialogue. The dialogues' table fills
/// as the records' entries of type 40 run, the first entry twice (`0x476A05`), so that a
/// dialogue's index is its entry's place from 1 and the index 0 gives none.
fn set_states(
    records: &[&Record],
    made: &HashMap<*const Record, usize>,
    placements: &mut [Placement],
) {
    let mut named: HashMap<String, Vec<usize>> = HashMap::new();
    let mut dialogues: Vec<String> = Vec::new();
    for record in records {
        let ops = match &record.body {
            Body::Dialog { name, .. } => {
                dialogues.push(name.clone());
                if dialogues.len() == 1 {
                    dialogues.push(name.clone());
                }
                continue;
            }
            Body::Operands(ops) => ops,
            Body::Block(_) => continue,
        };
        match record.kind {
            CREATE_NPC => {
                let index = made.get(&std::ptr::from_ref(*record));
                if let (Some(name), Some(&index)) = (name_of(ops), index) {
                    named.entry(name).or_default().push(index);
                }
            }
            SET_NPC_STATE => {
                let Some(indices) = name_of(ops).and_then(|name| named.get(&name)) else {
                    continue;
                };
                let state = State::of(ops, &dialogues);
                for &index in indices {
                    if let Some(placement) = placements.get_mut(index) {
                        state.apply(placement);
                    }
                }
            }
            _ => {}
        }
    }
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
/// chests' cells"); no script of the start opens them, but a door that names its trigger does
/// as it opens (#372). Each with its trigger's name, lower case.
fn closed_cells(records: &[&Record]) -> Vec<(String, Spot)> {
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
        .filter_map(|ops| {
            name_of(ops)
                .filter(|name| closed.contains(name))
                .map(|name| (name, ops))
        })
        .flat_map(|(name, ops)| {
            ops.iter().filter_map(move |o| match (o.tag, &o.value) {
                (0x2A, Value::I32x3([x, y, floor])) => Some((name.clone(), (*x, *y, *floor))),
                _ => None,
            })
        })
        .collect()
}

/// The named positions of `records`: `DefPos` name, x, y.
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
    /// Reads the scripts of `tree` and the tables, and what of them the hero's template's items
    /// `worn` are ([`Self::hero_holds`]).
    pub fn read(assets: &Path, tree: &str, worn: &[u32]) -> Result<Self> {
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
        let classes: HashMap<u32, u8> = creatures.iter().map(|c| (c.id, c.class)).collect();
        let bases: HashMap<u32, fight::Base> = creatures
            .iter()
            .map(|c| (c.id, fight::Base::of(c)))
            .collect();
        // The balance, from which the fills' factors and the health's (none where it is lacking).
        let balance =
            fs::read_to_string(assets.join("tables").join(opensacred_data::tables::BALANCE))
                .ok()
                .and_then(|text| ron::from_str::<opensacred_data::tables::Balance>(&text).ok());
        let path = assets.join("tables").join(opensacred_data::tables::WEAPONS);
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        // The templates of the items' values (`wpmod.bin`), none where they cannot be read.
        let modifiers: Vec<opensacred_data::tables::WeaponModifier> = fs::read_to_string(
            assets
                .join("tables")
                .join(opensacred_data::tables::WEAPON_MODIFIERS),
        )
        .ok()
        .and_then(|text| ron::from_str(&text).ok())
        .unwrap_or_default();
        let weapons: Vec<Weapon> =
            ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        // A weapon's kind, by its type, which is its item's slot (docs/02 §6.2).
        let kinds: HashMap<u32, u8> = weapons.iter().map(|w| (w.id, w.bytes_1c[2])).collect();
        let models = model_files(assets)?;
        let textures = texture_files(assets)?;
        let records = start_records(&tree);
        // The records of the sectors' functions, which the start does not run, and those of
        // their `Enter` functions, whose creatures step otherwise.
        let in_sectors: HashSet<*const Record> = tree
            .functions
            .iter()
            .filter(|f| sector_function(&f.name).is_some())
            .flat_map(|f| &f.records)
            .map(std::ptr::from_ref)
            .collect();
        let enter: HashSet<*const Record> = tree
            .functions
            .iter()
            .filter(|f| f.name.ends_with("Enter") && sector_function(&f.name).is_some())
            .flat_map(|f| &f.records)
            .map(std::ptr::from_ref)
            .collect();
        let named = positions(records.iter().copied());
        let targets = goto_targets(&records);
        // The NPCs' models' clips by their slots, each model read once.
        let mut model_slots: HashMap<String, motions::Slots> = HashMap::new();
        let mut placements = Vec::new();
        // The placement that each record made, by the record.
        let mut made: HashMap<*const Record, usize> = HashMap::new();
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
            // The items in its hands, whose values it fights with.
            let holding = InHand::of(
                equipped
                    .iter()
                    .map(|item| (item.slot, category(item), kinds.get(&item.slot).copied())),
            );
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
                let walk = slots.clip(motions::Action::Walk, class);
                (ClipNames::of(slots, class), walk.map(|(_, gait)| gait))
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
                    (to, f32::from(speed))
                });
            let [walking, running] = speeds.get(&creature).copied().unwrap_or_default();
            let tag = if record.kind == CREATE_NPC {
                level_tag(ops)
            } else {
                0
            };
            // Its flags word and its side by its operand-less tags: its mode, and whether it is
            // hostile or peaceful (`0x08`, `0x0E`; #580).
            let markers = || {
                ops.iter()
                    .filter(|o| matches!(o.value, Value::Marker))
                    .map(|o| o.tag)
            };
            let word = crowd::flags_word(markers());
            let class = classes.get(&creature).copied().unwrap_or_default();
            let side =
                minds::created_side(class, creature, word, minds::created_hostility(markers()));
            placements.push(Placement {
                model: model.clone(),
                cell: (x, y),
                drawn: if record.kind == CREATE_OBJ {
                    drawn_at(ops)
                } else {
                    None
                },
                home: None,
                walk,
                speed: [walking, running].map(f32::from),
                class,
                base: bases.get(&creature).copied().unwrap_or_default(),
                held: if record.kind == CREATE_NPC {
                    held_class(&categories)
                } else {
                    held_class(&[])
                },
                ranged: record.kind == CREATE_NPC && ranged(&categories),
                size: arrows::size(item.words_14[0]),
                made: (enter.contains(&std::ptr::from_ref(*record)), true),
                started: !in_sectors.contains(&std::ptr::from_ref(*record)),
                dialogue: record.kind == CREATE_NPC
                    && ops.iter().any(|o| {
                        o.tag == DIALOGUE || (o.tag == STAMPED && matches!(o.value, Value::Marker))
                    }),
                trader: record.kind == CREATE_NPC
                    && ops
                        .iter()
                        .any(|o| TRADES.contains(&o.tag) && matches!(o.value, Value::Marker)),
                kept: record.kind == CREATE_NPC && ops.iter().any(|o| KEEPS.contains(&o.tag)),
                group: None,
                heading: facing_degrees(record.kind == CREATE_OBJ, heading, item.angle()),
                object: record.kind == CREATE_OBJ,
                casts: if record.kind == CREATE_OBJ {
                    category(item) == CHEST
                } else {
                    !shadowless.contains(&creature)
                },
                blocks: record.kind == CREATE_OBJ && category(item) == CHEST,
                door: record.kind == CREATE_OBJ && category(item) == DOOR,
                trigger: (record.kind == CREATE_OBJ && category(item) == DOOR)
                    .then(|| {
                        ops.iter().find_map(|o| match (o.tag, &o.value) {
                            (TRIGGER, Value::Str(name)) => Some(name.to_ascii_lowercase()),
                            _ => None,
                        })
                    })
                    .flatten(),
                creature: (
                    creature,
                    record_flags.get(&creature).copied().unwrap_or_default(),
                    word,
                ),
                side,
                texture,
                layer,
                equipment: if record.kind == CREATE_NPC {
                    equipment
                } else {
                    Vec::new()
                },
                holding: if record.kind == CREATE_NPC {
                    holding
                } else {
                    Vec::new()
                },
                rank: fight::Rank::of_tag(tag),
                tag,
                clips,
                plain: false,
            });
            made.insert(std::ptr::from_ref(*record), placements.len() - 1);
        }
        set_states(&records, &made, &mut placements);
        // What each creature type that has a model is made of, and what each type that the
        // sectors' fills make is placed as ([`template`]).
        let types: HashMap<u32, Kind> = creatures
            .iter()
            .filter_map(|creature| {
                let item = items.get(&creature.id)?;
                let model = models.get(&item.name.to_ascii_uppercase())?;
                Some((
                    creature.id,
                    Kind {
                        model: model.clone(),
                        texture: match item.words_08[0] {
                            0 => None,
                            number => textures.get(&number).cloned(),
                        },
                        size: arrows::size(item.words_14[0]),
                        speed: speeds
                            .get(&creature.id)
                            .copied()
                            .unwrap_or_default()
                            .map(f32::from),
                        class: creature.class,
                        base: bases.get(&creature.id).copied().unwrap_or_default(),
                        flags: creature.flags,
                        casts: !shadowless.contains(&creature.id),
                    },
                ))
            })
            .collect();
        let Sectors { values, fills } = sectors(&tree);
        let mut templates: HashMap<u32, Placement> = HashMap::new();
        // The fills' types, and the waters' crabs and fish (`spawning::swimmers`).
        let swimmers = std::iter::once(spawning::CRAB).chain(spawning::FISH);
        for creature in fills
            .values()
            .flatten()
            .flat_map(spawning::types)
            .chain(swimmers)
        {
            if templates.contains_key(&creature) {
                continue;
            }
            let Some(kind) = types.get(&creature) else {
                continue;
            };
            if !model_slots.contains_key(&kind.model) {
                let slots = motions::Slots::read(&assets.join(&kind.model))?;
                model_slots.insert(kind.model.clone(), slots);
            }
            templates.insert(
                creature,
                template(creature, kind, &model_slots[&kind.model]),
            );
        }
        let pools = pools(&records);
        // A fixed item, a record of `Weapon.pak` without an `items.pak` slot of its own, as the
        // heroes' templates' are, takes its parent's (`+0x24`), its model and its category, as the
        // client copies the parent's definition into the child (docs/02 §6.2; #488).
        let parents: HashMap<u32, u32> = weapons.iter().map(|w| (w.id, w.parent)).collect();
        let record = |slot: u32| {
            std::iter::successors(Some(slot), |slot| {
                parents.get(slot).copied().filter(|&parent| parent != 0)
            })
            .take(8)
            .find_map(|slot| items.get(&slot).copied())
        };
        // The pools' items, the weapons that an archer takes to melee and back (#396), and the
        // hero's (#486).
        let gear = pools
            .values()
            .flatten()
            .chain(&opensacred_rules::minds::SWITCHED)
            .chain(worn)
            .filter_map(|&slot| {
                let item = record(slot)?;
                let gear = Gear {
                    category: category(item),
                    kind: kinds.get(&slot).copied(),
                    model: models.get(&item.name.to_ascii_uppercase()).cloned(),
                    texture: match item.words_08[0] {
                        0 => None,
                        number => textures.get(&number).cloned(),
                    },
                    counts: item.byte_03 & 2 == 0,
                };
                Some((slot, gear))
            })
            .collect();
        let side = u32::try_from(SIDE)?;
        let mut by_sector: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (index, placement) in placements.iter().enumerate() {
            let (x, y) = placement.cell;
            by_sector
                .entry((x / side, y / side))
                .or_default()
                .push(index);
        }
        let mut closed: HashMap<(u32, u32), Vec<(String, Spot)>> = HashMap::new();
        let mut triggers: HashMap<String, Vec<Spot>> = HashMap::new();
        for (name, (x, y, floor)) in closed_cells(&records) {
            triggers
                .entry(name.clone())
                .or_default()
                .push((x, y, floor));
            if let (Ok(cx), Ok(cy)) = (u32::try_from(x), u32::try_from(y)) {
                closed
                    .entry((cx / side, cy / side))
                    .or_default()
                    .push((name, (x, y, floor)));
            }
        }
        Ok(Self {
            placements,
            by_sector,
            closed,
            triggers,
            opened: HashSet::new(),
            marked: HashSet::new(),
            values,
            fills,
            templates,
            types,
            classes,
            factors: factors(balance.as_ref()),
            fight: fight::Rules::of(balance.as_ref()),
            items: fight::Templates::new(&modifiers, &weapons),
            rules: rules(&tree),
            pools,
            gear,
            slots: model_slots,
            hits: hit_fractions(assets),
            instruments: INSTRUMENTS
                .iter()
                .map(|&(slot, _)| {
                    let item = items.get(&slot)?;
                    let model = models.get(&item.name.to_ascii_uppercase())?.clone();
                    let texture = match item.words_08[0] {
                        0 => None,
                        number => textures.get(&number).cloned(),
                    };
                    Some((model, texture))
                })
                .collect(),
            arrow: items.get(&ARROW).and_then(|item| {
                let model = models.get(&item.name.to_ascii_uppercase())?.clone();
                let texture = match item.words_08[0] {
                    0 => None,
                    number => textures.get(&number).cloned(),
                };
                Some((model, texture, arrows::size(item.words_14[0])))
            }),
            spawned: HashSet::new(),
            kept: Kept::default(),
            waters: Vec::new(),
        })
    }

    /// What the hero holds and wears of `worn`, its template's items, as its values take them
    /// (`0x5796A0`): a weapon and a shield by the hands its items go in, as an NPC's ([`hands`],
    /// `0x546F80`, `0x54A660`), and the values of every item's record, held or worn
    /// (`fight::ItemValues::of_record`; #486), but those whose values do not count
    /// ([`Gear::counts`], #490).
    pub fn hero_holds(&self, worn: &[Worn]) -> fight::Holds {
        let holding = self.hero_holding(worn);
        let class = class_of(&holding);
        fight::Holds {
            weapon: class != 0 || holding.iter().any(|held| held.hand == Hand::Right),
            shield: holding
                .iter()
                .any(|held| held.hand == Hand::Left && held.category == SHIELD),
            ranged: ranged_of(&holding),
            items: worn
                .iter()
                .filter(|worn| self.gear.get(&worn.item).is_none_or(|gear| gear.counts))
                .map(|worn| fight::ItemValues::of_record(&worn.words))
                .collect(),
        }
    }

    /// The items of `worn`, the hero's template's, that go in its hands, as an NPC's go ([`hands`]).
    fn hero_holding(&self, worn: &[Worn]) -> Vec<InHand> {
        InHand::of(worn.iter().filter_map(|worn| {
            let gear = self.gear.get(&worn.item)?;
            Some((worn.item, gear.category, gear.kind))
        }))
    }

    /// The class of what the hero holds of `worn`, its template's items ([`class_of`]), by which
    /// it picks its clips, and the models of those in its hands (#488).
    pub fn hero_gear(&self, worn: &[Worn]) -> (u8, Vec<Held>) {
        let holding = self.hero_holding(worn);
        let held = holding
            .iter()
            .filter_map(|held| {
                let gear = self.gear.get(&held.item)?;
                Some(Held {
                    model: gear.model.clone()?,
                    texture: gear.texture.clone(),
                    hand: held.hand,
                    shield: held.category == SHIELD,
                })
            })
            .collect();
        (class_of(&holding), held)
    }

    /// The arrows' model and texture, relative to the asset directory, and their size, where
    /// the arrows' model was converted.
    pub fn arrow(&self) -> Option<(&str, Option<&str>, f32)> {
        self.arrow
            .as_ref()
            .map(|(model, texture, size)| (model.as_str(), texture.as_deref(), *size))
    }

    /// The NPCs and the objects to place, and the cells of the closed triggers.
    pub fn counts(&self) -> (usize, usize, usize) {
        let objects = self.placements.iter().filter(|p| p.object).count();
        let closed = self.closed.values().map(Vec::len).sum();
        (self.placements.len() - objects, objects, closed)
    }
}

/// The townsfolk's `Creature.pak` class, whom the partners are chosen from (docs/re/ai.md).
const TOWNSFOLK_CLASS: u8 = 3;

/// The horses' `Creature.pak` class, whose level `CreateNPC` raises by the hero's riding
/// (`0x4266F0`, `fight::Raise::Horse`).
const HORSES_CLASS: u8 = 4;

/// The record of the generator of a script's creature's roll of its level, of its cell
/// ([`Npcs::scripted_level`]): the level's offset in the creature (`+0x3FE`).
const LEVEL_ROLL: usize = 0x3FE;

/// The creatures that the command `spawn` makes (#524).
#[derive(Clone, Debug, PartialEq)]
pub struct Spawn {
    /// Their type, `creatures.ron`'s id.
    pub creature: u32,
    /// The global cells they stand on, a creature each, and their floor, 0 the ground.
    pub cells: Vec<(u32, u32)>,
    pub floor: i32,
    /// Their level, else the one `CreateNPC` gives them, by their sector's values about the
    /// hero's (`fight::scripted_level`).
    pub level: Option<u32>,
    pub rank: fight::Rank,
    /// Hostile or peaceful, else by their type: peaceful the townsfolk, the animals that take a
    /// behaviour and the critters, hostile the others.
    pub hostile: Option<bool>,
    /// Whether they take no damage (`fight::Immortal`).
    pub immortal: bool,
}

impl Npcs {
    /// What a creature of type `creature` is placed as: the fills' template, else one made of
    /// its kind, its model's clips read from `assets` once.
    fn template(&mut self, creature: u32, assets: &Path) -> Result<Placement, String> {
        if let Some(template) = self.templates.get(&creature) {
            return Ok(template.clone());
        }
        let kind = self
            .types
            .get(&creature)
            .ok_or_else(|| format!("type {creature} has no model"))?
            .clone();
        if !self.slots.contains_key(&kind.model) {
            let slots =
                motions::Slots::read(&assets.join(&kind.model)).map_err(|e| format!("{e:#}"))?;
            self.slots.insert(kind.model.clone(), slots);
        }
        let made = template(creature, &kind, &self.slots[&kind.model]);
        self.templates.insert(creature, made.clone());
        Ok(made)
    }

    /// The level that `CreateNPC` gives the creature of `placement`: its level tag's, held by
    /// its cell's sector's highest, or a boss's marker level (`fight::tagged_level`, #540); else
    /// (`fight::scripted_level`) the hero's, `hero`, by the levels of its cell's sector, then a
    /// horse's by the hero's riding and any other's by the time played, `minutes`, held at the
    /// sector's highest (#562); its roll the same each time it is made on its cell.
    fn scripted_level(&self, placement: &Placement, hero: u32, minutes: u32) -> u32 {
        let (x, y) = placement.cell;
        let side = u32::try_from(SIDE).unwrap_or(64);
        let values = self.values.get(&(x / side, y / side));
        let horse = placement.class == HORSES_CLASS;
        let creature = placement.creature.0;
        if let Some(level) =
            fight::tagged_level(placement.tag, values, creature, horse, &self.fight)
        {
            return level;
        }
        let raise = if horse {
            // The hero's skills stay at level 1 (`fight::skills`), an eighth of which is none.
            fight::Raise::Horse { riding: 0 }
        } else {
            fight::Raise::Played { minutes }
        };
        let mut dice = spawning::Rng::of(x, y, LEVEL_ROLL);
        fight::scripted_level(values, hero, raise, creature, &self.fight, &mut dice)
    }
}

/// Spawns the creatures of `request`, as `CreateNPC` makes its own (awake when hostile), with
/// their hands empty, kept by their sectors as a fill's are; the creatures.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
pub fn spawn_creatures(
    In(request): In<Spawn>,
    mut commands: Commands,
    mut npcs: ResMut<Npcs>,
    mut streamer: ResMut<Streamer>,
    heights: Res<Heights>,
    states: Res<buildings::Buildings>,
    assets: Res<AssetServer>,
    data: Res<crate::game::Data>,
    experience: Res<fight::Experience>,
    played: Res<drops::Played>,
) -> Result<Vec<Entity>, String> {
    let npcs = &mut *npcs;
    let creature = request.creature;
    let mut placement = npcs.template(creature, &data.0.assets)?;
    let critter = crowd::constructed(creature, placement.creature.1);
    let hostile = request.hostile.unwrap_or(
        placement.class != TOWNSFOLK_CLASS && ai::of_type(creature) == 0 && critter == 0,
    );
    placement.made = (false, true);
    placement.layer = request.floor;
    placement.rank = request.rank;
    placement.creature.2 = critter
        | if hostile {
            crowd::HOSTILE
        } else {
            ai::of_type(creature)
        };
    placement.side = minds::made_side(placement.class, creature, hostile);
    let hero = experience.level;
    let side = u32::try_from(SIDE).unwrap_or(64);
    let mut made = Vec::new();
    for &cell in &request.cells {
        placement.cell = cell;
        let at = (cell.0 / side, cell.1 / side);
        let values = npcs.values.get(&at).copied().unwrap_or_default();
        let level = request.level.unwrap_or_else(|| {
            let level = npcs.scripted_level(&placement, hero, played.minutes());
            let big = placement.creature.1 & crowd::BIG != 0;
            placement.rank.level(level, hero, &values, big, &npcs.fight)
        });
        let entity = spawn(
            &mut commands,
            &placement,
            &heights,
            &states,
            &assets,
            (level, &npcs.fight, &npcs.items),
        );
        if request.immortal {
            commands.entity(entity).insert(fight::Immortal);
        }
        let placed = Placed {
            at,
            source: Source::Filled(Box::new(placement.clone())),
        };
        npcs.kept.spawned(&placed);
        commands.entity(entity).insert(placed);
        streamer.adopt(at, entity);
        made.push(entity);
    }
    Ok(made)
}

/// Places the NPCs of every sector that came in, as entities of that sector, and marks the cells
/// that its objects take.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
pub fn place(
    mut commands: Commands,
    mut npcs: ResMut<Npcs>,
    mut streamer: ResMut<Streamer>,
    mut heights: ResMut<Heights>,
    states: Res<buildings::Buildings>,
    dungeons: Res<daynight::Dungeons>,
    assets: Res<AssetServer>,
    experience: Res<fight::Experience>,
    played: Res<drops::Played>,
) {
    let npcs = &mut *npcs;
    let hero = experience.level;
    let minutes = played.minutes();
    let side = u32::try_from(SIDE).unwrap_or(64);
    let loaded: Vec<(u32, u32)> = streamer.sectors().collect();
    for &at in &loaded {
        if npcs.spawned.contains(&at) {
            continue;
        }
        npcs.spawned.insert(at);
        // The closed triggers' cells: of the ground for the floors 0 and 1, of the building's
        // floor for 1 and above (`0x463F50`, which keys the floors 0 and 1 alike).
        for (name, spot) in npcs.closed.get(&at).map(Vec::as_slice).unwrap_or_default() {
            if !npcs.opened.contains(name) {
                for (cell, level) in spot_levels(&heights, *spot) {
                    heights.occupy(cell, level);
                }
            }
        }
        for &index in npcs
            .by_sector
            .get(&at)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            let placement = &npcs.placements[index];
            // The start's and the `Init`s' creatures persist; the `Enter`s' are made again.
            let kept = !placement.object && !placement.made.0;
            let again = if kept {
                npcs.kept.again(index)
            } else {
                Again::Anew
            };
            let entity = match again {
                Again::Not => continue,
                Again::Anew => {
                    // The start's creature as the game starts, with the hero of its first level
                    // and no time played; a sector's as its function runs; a script's boss then
                    // at the hero's level now and more, as a fill's (`0x550E50`).
                    let level = if placement.started {
                        npcs.scripted_level(placement, fight::HERO_LEVEL, 0)
                    } else {
                        npcs.scripted_level(placement, hero, minutes)
                    };
                    let values = npcs.values.get(&at).copied().unwrap_or_default();
                    let big = placement.creature.1 & crowd::BIG != 0;
                    let level = placement.rank.level(level, hero, &values, big, &npcs.fight);
                    spawn(
                        &mut commands,
                        placement,
                        &heights,
                        &states,
                        &assets,
                        (level, &npcs.fight, &npcs.items),
                    )
                }
                Again::AtStood(stood) => respawn(
                    &mut commands,
                    placement,
                    stood,
                    (&heights, &states, &assets),
                    (&npcs.fight, &npcs.items),
                ),
            };
            if kept {
                let placed = Placed {
                    at,
                    source: Source::Kept(index),
                };
                npcs.kept.spawned(&placed);
                commands.entity(entity).insert(placed);
            }
            streamer.adopt(at, entity);
        }
        // The fills' creatures that live: those that went with the sector come back where they
        // stood, and the sector fills anew only when none lives (`0x4939F0`).
        let (back, anew) = npcs.kept.back(at);
        for (placement, stood) in back {
            let entity = respawn(
                &mut commands,
                &placement,
                stood,
                (&heights, &states, &assets),
                (&npcs.fight, &npcs.items),
            );
            let placed = Placed {
                at,
                source: Source::Filled(Box::new(placement)),
            };
            npcs.kept.spawned(&placed);
            commands.entity(entity).insert(placed);
            streamer.adopt(at, entity);
        }
        if !anew {
            continue;
        }
        // The crabs and fish of its waters (`0x4939F0`), once the sectors about it are in.
        if !npcs.waters.contains(&at) {
            npcs.waters.push(at);
        }
        // The creatures that the sector's fills make, on cells at random that they may stand
        // on, the same ones each time the sector comes in (`spawning`).
        let Some(fills) = npcs.fills.get(&at) else {
            continue;
        };
        let values = npcs.values.get(&at).copied().unwrap_or_default();
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
            let count = spawning::count(fill, values, hero, factor, &mut rng);
            // One level for all the creatures of the run, before they are made (`0x47D187`, #582).
            let shared = fight::fill_level(&values, hero, minutes, &mut rng);
            let ground = spawning::Ground {
                origin: (x0, y0),
                dungeon: dungeons.sector(at),
                class: |creature| npcs.classes.get(&creature).copied(),
                walkable: |cell| heights.walkable_on(cell, None),
                flags: |cell| heights.flags(cell),
            };
            for made in spawning::make(fill, count, &ground, &mut rng) {
                let (Some(template), Ok(cx), Ok(cy)) = (
                    npcs.templates.get(&made.creature),
                    u32::try_from(made.cell.0),
                    u32::try_from(made.cell.1),
                ) else {
                    continue;
                };
                let mut placement = template.clone();
                placement.cell = (cx, cy);
                // Its items by the sector's rules, in its hands, and its clips by what it holds.
                let rules = npcs.rules.get(&at).map(Vec::as_slice).unwrap_or_default();
                let gear = gear_of(rules, made.creature, &npcs.pools, &npcs.gear, &mut rng);
                if !gear.is_empty() {
                    let categories: Vec<(u8, Option<u8>)> =
                        gear.iter().map(|(_, g)| (g.category, g.kind)).collect();
                    placement.holding =
                        InHand::of(gear.iter().map(|&(item, g)| (item, g.category, g.kind)));
                    placement.equipment = gear
                        .iter()
                        .map(|(_, g)| *g)
                        .zip(hands(&categories))
                        .filter_map(|(g, hand)| {
                            Some(Held {
                                model: g.model.clone()?,
                                texture: g.texture.clone(),
                                hand: hand?,
                                shield: g.category == SHIELD,
                            })
                        })
                        .collect();
                    placement.held = held_class(&categories);
                    placement.ranged = ranged(&categories);
                    if let Some(slots) = npcs.slots.get(&placement.model) {
                        placement.clips = ClipNames::of(slots, placement.held);
                    }
                }
                // A random heading, as the creature maker gives one (`rand() % 361`).
                placement.heading = f32::from(u16::try_from(rng.below(361)).unwrap_or(0));
                // A hostile one bit 0, a peaceful one its type's behaviour (`0x480D40`), and a
                // critter its constructor's bit (`0x52DE40`), which no `CreateNPC` writes over.
                placement.creature.2 |= crowd::constructed(made.creature, placement.creature.1)
                    | if made.hostile {
                        crowd::HOSTILE
                    } else {
                        ai::of_type(made.creature)
                    };
                // Its side as the creature maker leaves it for the entry's hostility (#580).
                placement.side = minds::made_side(placement.class, made.creature, made.hostile);
                // A pack's group, by its sector, its record and its leader.
                placement.group = made.pack.and_then(|leader| {
                    let run = u32::try_from(index * 100 + leader).ok()? % 1000;
                    Some((at.0 % 100 * 100 + at.1 % 100) * 1000 + run)
                });
                // A boss or a champion, at the hero's level and more (`0x47E6B7`, `0x550E50`);
                // any other at the run's level, one or two more on a dungeon's sector (#570),
                // held at its sector's highest as the creature maker sets it (#562), a horse's
                // not.
                placement.rank =
                    fight::Rank::of_fill(made.creature, made.hostile, &mut rng, &npcs.fight);
                let level = if placement.rank == fight::Rank::Common {
                    let horse = placement.class == HORSES_CLASS;
                    let dungeon = dungeons.sector((cx / side, cy / side));
                    let creature = (made.creature, horse);
                    fight::filled_level(shared, dungeon, &values, creature, &npcs.fight, &mut rng)
                } else {
                    let big = placement.creature.1 & crowd::BIG != 0;
                    placement.rank.level(1, hero, &values, big, &npcs.fight)
                };
                let entity = spawn(
                    &mut commands,
                    &placement,
                    &heights,
                    &states,
                    &assets,
                    (level, &npcs.fight, &npcs.items),
                );
                let placed = Placed {
                    at,
                    source: Source::Filled(Box::new(placement)),
                };
                npcs.kept.spawned(&placed);
                commands.entity(entity).insert(placed);
                streamer.adopt(at, entity);
            }
        }
    }
    // The waters' crabs and fish of the sectors come in anew, the same ones each time, once no
    // sector is building: their cells' rings may reach into the next sectors.
    if streamer.building() == 0 {
        for at in std::mem::take(&mut npcs.waters) {
            if loaded.contains(&at) {
                swimmers(
                    &mut commands,
                    npcs,
                    &mut streamer,
                    at,
                    (&heights, &states, &assets),
                    &dungeons,
                );
            }
        }
    }
    // Sectors that went out place their NPCs again when they come back, as [`Kept`] has it.
    npcs.spawned.retain(|at| loaded.contains(at));
}

/// The level of the crabs and fish, which no level's setter raises (`Placement::plain`).
const PLAIN_LEVEL: u32 = 1;
/// Their most health, the stats' own (seen in the original, #575).
const PLAIN_HEALTH: u32 = 100;

/// Spawns the crabs and fish of the waters of sector `at`, come in anew (`0x4939F0`,
/// `spawning::swimmers`): of its liquid materials, its area and whether it is a dungeon, the
/// cells of the loaded sectors alone, each at level 1, peaceful, facing its way. A crab, which
/// no `Creature.pak` record describes, the game has no template of, and makes none.
fn swimmers(
    commands: &mut Commands,
    npcs: &mut Npcs,
    streamer: &mut Streamer,
    at: (u32, u32),
    (heights, states, assets): (&Heights, &buildings::Buildings, &AssetServer),
    dungeons: &daynight::Dungeons,
) {
    let side = u32::try_from(SIDE).unwrap_or(64);
    let (Some(liquids), Ok(x0), Ok(y0)) = (
        heights.liquids(at),
        i32::try_from(at.0 * side),
        i32::try_from(at.1 * side),
    ) else {
        return;
    };
    let waters = spawning::Waters {
        origin: (x0, y0),
        liquids,
        area: streamer.area(at).unwrap_or(0),
        dungeon: dungeons.sector(at),
        bed: |cell| heights.liquid_bed(cell),
    };
    // A generator of its own, apart from the fills' records'.
    let mut rng = spawning::Rng::of(at.0, at.1, usize::MAX);
    for swimmer in spawning::swimmers(&waters, &mut rng) {
        let (Some(template), Ok(cx), Ok(cy)) = (
            npcs.templates.get(&swimmer.creature),
            u32::try_from(swimmer.cell.0),
            u32::try_from(swimmer.cell.1),
        ) else {
            bevy::log::debug!(
                "npcs: no {} in the water of {},{}",
                swimmer.creature,
                swimmer.cell.0,
                swimmer.cell.1
            );
            continue;
        };
        let mut placement = template.clone();
        placement.cell = (cx, cy);
        placement.plain = true;
        // Its direction on the grid (`0x44DD90`) as the screen's angle that turns to it.
        let (gx, gy) = swimmer.facing;
        placement.heading = (gx + gy).atan2(2.0 * (gx - gy)).to_degrees();
        // Its constructor's bits alone, no behaviour: the creature maker does not make it.
        placement.creature.2 = crowd::constructed(swimmer.creature, placement.creature.1);
        let entity = spawn(
            commands,
            &placement,
            heights,
            states,
            assets,
            (PLAIN_LEVEL, &npcs.fight, &npcs.items),
        );
        let placed = Placed {
            at,
            source: Source::Filled(Box::new(placement)),
        };
        npcs.kept.spawned(&placed);
        commands.entity(entity).insert(placed);
        streamer.adopt(at, entity);
    }
}

/// Spawns `placement`'s creature where it stood when it went with its sector, on that floor,
/// with the health and the level it had.
fn respawn(
    commands: &mut Commands,
    placement: &Placement,
    stood: Stood,
    (heights, states, assets): (&Heights, &buildings::Buildings, &AssetServer),
    (rules, templates): (&fight::Rules, &fight::Templates),
) -> Entity {
    let mut there = placement.clone();
    there.cell = stood.cell;
    there.layer = stood.layer;
    bevy::log::debug!("npcs: {} back {stood:?}", placement.model);
    let entity = spawn(
        commands,
        &there,
        heights,
        states,
        assets,
        (stood.health.level, rules, templates),
    );
    commands.entity(entity).insert(stood.health);
    entity
}

/// Notes what a placed creature leaves as it goes with its sector, or with the game (#329): a
/// kept one's place, floor and health, or that it was killed; a fill's living one, with its
/// sector.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn keep(
    removed: On<Remove<Placed>>,
    creatures: Query<(
        &Placed,
        &Transform,
        &crowd::InTheWay,
        Option<&fight::Health>,
        Has<Dies>,
    )>,
    heights: Option<Res<Heights>>,
    npcs: Option<ResMut<Npcs>>,
) {
    let (Ok((placed, transform, way, health, dies)), Some(mut npcs)) =
        (creatures.get(removed.entity), npcs)
    else {
        return;
    };
    let (x, y) = paths::cell_of(transform.translation.x, transform.translation.z);
    let layer = heights
        .as_deref()
        .and_then(|heights| heights.floor(way.level))
        .map_or(0, |(_, floor)| i32::from(floor));
    let stood = match (u32::try_from(x), u32::try_from(y), health) {
        (Ok(x), Ok(y), Some(&health)) if !dies && health.current > 0 => Some(Stood {
            cell: (x, y),
            layer,
            health,
        }),
        _ => None,
    };
    bevy::log::debug!(
        "npcs: {} of {:?} goes, {stood:?}",
        removed.entity,
        placed.at
    );
    npcs.kept.gone(placed, stood);
}

/// Spawns the NPC or the object of `placement`: its model on its cell and level, an object's at
/// its drawn position (`model_position`), turned to its heading, with its texture, its items, its
/// shadow, its clips, its walk and its place in the others' way.
fn spawn(
    commands: &mut Commands,
    placement: &Placement,
    heights: &Heights,
    states: &buildings::Buildings,
    assets: &AssetServer,
    // The creature's level, the balance's values of its fight, and the templates of the items'
    // values.
    (grade, rules, templates): (u32, &fight::Rules, &fight::Templates),
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
    // Its model where its record draws it, an object's where its `0x20` shows.
    let model = model_position(position, placement.drawn);
    let off = Vec2::new(model.x - position.x, model.z - position.z).length();
    if off > 0.0 {
        bevy::log::debug!(
            "npcs: {} of cell {x},{y} drawn at {:.0},{:.0}, {off:.0} from its cell's centre",
            placement.model,
            model.x,
            model.z,
        );
    }
    let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(placement.model.clone()));
    let mut entity = commands.spawn((
        WorldAssetRoot(scene),
        Transform::from_translation(model)
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
        entity.insert(equipment(assets, &placement.equipment));
    }
    if placement.casts {
        entity.insert(shadows::Casts);
    }
    if placement.object {
        entity.insert(Object { at: position });
        if placement.blocks {
            entity.insert(crowd::InTheWay::object(level));
        }
        if placement.door {
            entity.insert(Door {
                gltf: assets.load(placement.model.clone()),
                clips: None,
                player: None,
                open: false,
                trigger: placement.trigger.clone(),
                along: {
                    let along = heading(placement.heading);
                    Vec2::new(along.x, along.z)
                },
            });
        }
    } else {
        let (creature, record_flags, word) = placement.creature;
        // Its home, where a `SetNPCState` gives one, else where it is made.
        let home = placement.home.map_or(position, |(hx, hy)| {
            iso::ground(hx as f32 + 0.5, hy as f32 + 0.5)
        });
        let mut mind = ai::Mind::new(
            home,
            word,
            placement.made,
            placement.group,
            placement.speed,
            (creature, placement.class),
        );
        mind.hold(placement.dialogue, placement.trader, placement.kept);
        mind.side = placement.side;
        bevy::log::debug!(
            "npcs: {} is a {creature} of the flags {word:#x}, {:#x} after its first update",
            entity.id(),
            mind.flags
        );
        mind.level = grade;
        mind.ranged = placement.ranged;
        // Its health, full, by its base, its level, its skill 10 and its rank (`0x5658F0`); a
        // crab's or a fish's the stats' own.
        let skills = fight::skills(&placement.base, grade, placement.plain);
        let rank = placement.rank.health(creature, placement.class, rules);
        let most = if placement.plain {
            PLAIN_HEALTH
        } else {
            fight::most_health(placement.base.attributes, grade, &skills, rank, rules)
        };
        entity.insert(fight::Health {
            current: most,
            most,
            level: grade,
        });
        // Its attack, defence, damage and resistances with the values of its items, made at its
        // level (`0x5D5080`), the same each time it is made, its hits' share, and its
        // regeneration.
        let mut dice = spawning::Rng::of(x, y, usize::try_from(creature).unwrap_or(0));
        let arms = Arms {
            base: placement.base,
            plain: placement.plain,
            model: placement.model.clone(),
            holding: placement
                .holding
                .iter()
                .map(|&held| (held, templates.make(held.item, grade, &mut dice)))
                .collect(),
        };
        for (held, values) in &arms.holding {
            if let Some(values) = values {
                bevy::log::debug!("npcs: {creature} holds {}: {values:?}", held.item);
            }
        }
        let (values, share) = arms.fight(grade, placement.rank, rules);
        // Its walk and run by its pace (`0x4F75F0`, #599).
        let pace = values.0.pace_factor();
        mind.speed = placement.speed[0] * pace;
        mind.run = placement.speed[1] * pace;
        if placement.rank != fight::Rank::Common {
            bevy::log::debug!(
                "npcs: {creature} is a {:?} of level {grade}",
                placement.rank
            );
        }
        entity.insert((
            values,
            arms,
            fight::Ranked(placement.rank),
            fight::Worth(placement.base.experience),
            fight::Regenerates::new(fight::regeneration(&placement.base, grade, &skills, rules)),
        ));
        entity.insert((
            Npc {
                gltf: assets.load(placement.model.clone()),
                names: placement.clips.clone(),
                graph: None,
                player: None,
                hands_empty: placement.equipment.is_empty(),
                instrument: None,
                share,
                pace,
            },
            crowd::InTheWay::of(record_flags, word, level),
            mind,
            arrows::Size(placement.size),
        ));
        if let Some(((tx, ty), speed)) = placement.walk {
            entity.insert(Walk::to(
                iso::ground(tx as f32 + 0.5, ty as f32 + 0.5),
                level,
                speed * pace,
            ));
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

/// The model of an item that an NPC holds, attached to its hand (`equip`): the hand, its holder
/// being the NPC of its `models::OrderedWith`.
#[derive(Component, Clone, Copy, Debug)]
pub struct Wielded(Hand);

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
    /// The frames it waits once its holder's scene is ready ([`SETTLES`]).
    waits: u8,
}

/// How many frames an item waits after its holder's scene is ready before it is attached
/// ([`equip`]): Bevy's spawner may spawn a scene's instance anew within its first two frames
/// (`WORLD_ASSET_AGE_THRESHOLD`), despawning the bones of the holder and the items on them, and a
/// spawn of an item's scene still queued then panics on its despawned root (#488).
const SETTLES: u8 = 4;

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
/// The equipment of the items `held`, their models and textures loading, to attach once its
/// holder's scene is ready ([`equip`]).
pub fn equipment(assets: &AssetServer, held: &[Held]) -> Equipment {
    Equipment(
        held.iter()
            .map(|held| Equip {
                gltf: assets.load(held.model.clone()),
                texture: held.texture.as_ref().map(|t| load_texture(assets, t)),
                hand: held.hand,
                shield: held.shield,
                ready: false,
                waits: SETTLES,
            })
            .collect(),
    )
}

pub fn equipment_ready(ready: On<WorldInstanceReady>, mut npcs: Query<&mut Equipment>) {
    if let Ok(mut equipment) = npcs.get_mut(ready.entity) {
        for item in &mut equipment.0 {
            item.ready = true;
        }
    }
}

/// Attaches each NPC's equipment, once its scene is ready and the item's glTF and its scene are
/// loaded: the item's socket bone (see [`bones`]) goes onto the NPC's bone that holds it. An item
/// whose model lacks its socket is not drawn. The item's scene is spawned only once it is loaded,
/// so that it spawns at once: a spawn put off to a later frame panics in Bevy's spawner when its
/// holder is despawned meanwhile, as a hero's is by a new game started at once (#488).
pub fn equip(
    mut commands: Commands,
    mut npcs: Query<(Entity, &mut Equipment)>,
    gltfs: Res<Assets<Gltf>>,
    worlds: Res<Assets<WorldAsset>>,
    nodes: Res<Assets<GltfNode>>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    for (npc, mut equipment) in &mut npcs {
        equipment.0.retain_mut(|item| {
            if !item.ready {
                return true;
            }
            if item.waits > 0 {
                item.waits -= 1;
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
            if !worlds.contains(&scene) {
                return true;
            }
            let mut weapon = commands.spawn((
                WorldAssetRoot(scene),
                Transform::from_matrix(pose.to_matrix().inverse()),
                ChildOf(holder),
                models::OrderedWith(npc),
                Wielded(item.hand),
            ));
            if let Some(texture) = &item.texture {
                weapon.insert(Retexture(texture.clone()));
            }
            bevy::log::debug!("npcs: {npc} wields {} in {holder}", weapon.id());
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
/// model's first, and its walk and run clips, which play while the NPC walks or runs.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
pub fn animate(
    mut commands: Commands,
    data: Res<Npcs>,
    mut npcs: Query<(&mut Npc, Option<&Walk>)>,
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
        let Some((mut npc, walk)) = npc else {
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
            let mut graph = AnimationGraph::new();
            let root = graph.root;
            let idle = graph.add_clip(idle, 1.0, root);
            let mut add = |name| clip(name).map(|clip| graph.add_clip(clip.clone(), 1.0, root));
            let (walk, run, stance, talk) = (
                add(&npc.names.walk),
                add(&npc.names.run),
                add(&npc.names.stance),
                add(&npc.names.talk),
            );
            let names = npc.names.fun.clone();
            let fun = [0, 1, 2, 3].map(|i| add(&names[i]));
            let dying = npc
                .names
                .dying
                .iter()
                .filter_map(|name| {
                    let clip = gltf.named_animations.get(name.as_str())?;
                    Some(graph.add_clip(clip.clone(), 1.0, root))
                })
                .collect();
            let attacks = npc
                .names
                .attacks
                .iter()
                .filter_map(|name| {
                    let clip = gltf.named_animations.get(name.as_str())?.clone();
                    let hits = data
                        .hits
                        .get(&name.to_ascii_uppercase())
                        .copied()
                        .unwrap_or_default();
                    Some((graph.add_clip(clip.clone(), 1.0, root), clip, hits))
                })
                .collect();
            npc.graph = Some(Clips {
                graph: graphs.add(graph),
                idle,
                walk,
                run,
                stance,
                talk,
                fun,
                attacks,
                dying,
            });
        }
        if let Some(clips) = npc.graph.clone() {
            let moving = walk.and_then(|walk| clips.moving(walk));
            let node = moving.unwrap_or(clips.idle);
            // The clips change through transitions, which blend them (`stance`); a walk's and a
            // run's play at its pace too (`0x413610`, #599).
            let rate = if moving.is_some() { npc.pace } else { 1.0 };
            let mut transitions = AnimationTransitions::new();
            transitions
                .play(&mut player, node, Duration::ZERO)
                .repeat()
                .set_speed(speed.0 * rate);
            commands
                .entity(entity)
                .insert((AnimationGraphHandle(clips.graph), transitions));
            npc.player = Some(entity);
        }
    }
}

/// Walks each walking NPC toward its target as the hero walks (`hero::run`): round what blocks
/// the way and through the doors, at its walk speed times the game's, facing its way; at the
/// target it stands, its idle clip playing. A target beyond 25 cells along an axis it heads for
/// a leg at a time, its way searched to the cell brought nearer along the line (`paths::leg`,
/// `0x4FABE0`, #425). A target it cannot reach it walks straight toward, up to what blocks it,
/// and stands there; one whose first step is blocked stands a second in its idle before its walk
/// ends (#258).
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
        if walk.balked > 0.0 {
            walk.balked -= time.delta_secs();
            if walk.balked <= 0.0 {
                commands.entity(entity).remove::<Walk>();
            }
            continue;
        }
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
            walk.moved = true;
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
        let leg = paths::leg(here, target, |cell| heights.walkable_on(cell, level));
        let toward = match paths::next(
            here,
            (level, false),
            leg,
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
        if left <= walk.within || blocked || distance <= f32::EPSILON {
            // There, or as near as it gets: it stands; held from its first step, a while.
            if walk.moved || left <= walk.within {
                commands.entity(entity).remove::<Walk>();
            } else {
                walk.balked = BALKS;
            }
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
        walk.moved = true;
    }
}

/// How long a standing creature's clip blends into the next (`0x593320`).
const BLEND: Duration = Duration::from_millis(150);
/// How near the hero's nearest hostile puts the standing creatures in their fight stance, in
/// world units (`0x546A7E`).
const STANCE_WITHIN: f32 = 550.0;

/// Puts each walking NPC in its walk or run clip, and each standing NPC whose model has a fight
/// stance into it while the hero's nearest hostile is within 550 units, wherever the NPC stands,
/// and back into its idle otherwise, blending over 0.15 s (`0x5467A0`, which a standing creature
/// asks at every update, `0x544890`; docs/re/motions.md, "The fight stance").
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
pub fn stance(
    mut commands: Commands,
    nearest: Res<crowd::Nearest>,
    npcs: Query<
        (Entity, &Npc, Option<&Walk>, Has<Talks>, Option<&Plays>),
        (Without<Strikes>, Without<Dies>),
    >,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    speed: Res<hero::GameSpeed>,
) {
    let fight = nearest.distance < STANCE_WITHIN;
    for (entity, npc, walk, talks, plays) in &npcs {
        let (Some(clips), Some(player)) = (&npc.graph, npc.player) else {
            continue;
        };
        let Ok((mut player, mut transitions)) = players.get_mut(player) else {
            continue;
        };
        // A standing NPC that talks plays its talk once, then stands as before.
        if talks {
            match (walk, clips.talk) {
                (None, Some(talk)) if transitions.get_main_animation() != Some(talk) => {
                    transitions
                        .play(&mut player, talk, BLEND)
                        .set_speed(speed.0);
                    continue;
                }
                (None, Some(talk)) if !player.animation(talk).is_some_and(|a| a.is_finished()) => {
                    continue;
                }
                _ => {
                    commands.entity(entity).remove::<Talks>();
                }
            }
        }
        // A standing juggler plays its instrument's clip, once or on and on.
        if let Some(plays) = plays {
            let slot = npc.instrument.map_or(FUN, |i| INSTRUMENTS[i].1);
            let node = usize::from(slot - FUN);
            match (walk, clips.fun.get(node).copied().flatten()) {
                (None, Some(fun)) if transitions.get_main_animation() != Some(fun) => {
                    let active = transitions.play(&mut player, fun, BLEND);
                    if plays.on {
                        active.repeat();
                    }
                    active.set_speed(speed.0);
                    continue;
                }
                (None, Some(fun))
                    if plays.on || !player.animation(fun).is_some_and(|a| a.is_finished()) =>
                {
                    continue;
                }
                _ => {
                    commands.entity(entity).remove::<Plays>();
                }
            }
        }
        // A walking NPC walks; a standing one stands, in its stance where it fights.
        let moving = walk.and_then(|walk| clips.moving(walk));
        let wanted = match (moving, fight, clips.stance) {
            (Some(moving), _, _) => moving,
            (None, true, Some(stance)) => stance,
            _ => clips.idle,
        };
        if transitions.get_main_animation() != Some(wanted) {
            let rate = if moving.is_some() { npc.pace } else { 1.0 };
            transitions
                .play(&mut player, wanted, BLEND)
                .repeat()
                .set_speed(speed.0 * rate);
        }
    }
}

/// A door, which the townsfolk go into the houses and out of them by (`ai`): its model, its
/// clips of opening and closing (`*_ACTIVATE`, `*_DEACTIVATE`) and its animation player once
/// it has them, and whether it is open (bit 0 of its trigger's word, `0x4FC8D0`).
#[derive(Component)]
pub struct Door {
    gltf: Handle<Gltf>,
    clips: Option<(AnimationNodeIndex, AnimationNodeIndex)>,
    player: Option<Entity>,
    pub open: bool,
    /// The trigger it names, whose cells open and close with it (#372).
    trigger: Option<String>,
    /// The direction of its leaf on the ground, along its type's heading (docs/02 §5.7).
    pub along: Vec2,
}

impl Npcs {
    /// The cell of `door`, its trigger's (docs/02 §5.7: one cell each, with its floor), if its
    /// trigger is patched on one.
    pub fn door_cell(&self, door: &Door) -> Option<Spot> {
        let name = door.trigger.as_ref()?;
        self.triggers.get(name)?.first().copied()
    }

    /// Whether a hero has turned `door` (#372): its trigger keeps the hero's mark, the flags'
    /// bit `0x2000` of the original's door (`0x44BCD0`), which the creatures in its way look for
    /// (`0x4D4800`).
    pub fn marked(&self, door: &Door) -> bool {
        door.trigger
            .as_ref()
            .is_some_and(|name| self.marked.contains(name))
    }
}

/// A door that `ai` or the hero (`doors`) opens or closes, as the event `0x21` turns it
/// (`0x4FC8D0`, `0x44BCD0`): the event of a creature that is no hero leaves open a door that a
/// hero turned before.
#[derive(Component)]
pub struct Swing {
    pub open: bool,
    by_hero: bool,
}

impl Swing {
    /// A townsperson's opening (`true`) or closing of a door.
    pub fn to(open: bool) -> Self {
        Self {
            open,
            by_hero: false,
        }
    }

    /// The hero's.
    pub fn by_hero(open: bool) -> Self {
        Self {
            open,
            by_hero: true,
        }
    }
}

/// The cells and levels that a trigger's cell `(x, y, floor)` closes: the ground's for the
/// floors 0 and 1, the building's floor's for 1 and above (`0x463F50`, which keys the floors 0
/// and 1 alike).
fn spot_levels(heights: &Heights, (x, y, floor): Spot) -> Vec<((i32, i32), Level)> {
    let mut levels = Vec::new();
    if floor <= 1 {
        levels.push(((x, y), None));
    }
    if let Ok(floor) = usize::try_from(floor)
        && floor >= 1
        && let Some(building) = heights.building_on_floor((x, y), floor)
        && let Some(level) = heights.floor_level(building, floor)
    {
        levels.push(((x, y), Some(level)));
    }
    levels
}

/// Gives each door's animation player its clips of opening and closing.
fn door_clips(
    mut commands: Commands,
    mut doors: Query<&mut Door>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    players: Query<Entity, (With<AnimationPlayer>, Without<AnimationGraphHandle>)>,
    parents: Query<&ChildOf>,
) {
    for entity in &players {
        // The door this player belongs to: the nearest ancestor with `Door`.
        let mut ancestor = entity;
        let door = loop {
            if doors.contains(ancestor) {
                break Some(ancestor);
            }
            match parents.get(ancestor) {
                Ok(parent) => ancestor = parent.parent(),
                Err(_) => break None,
            }
        };
        let Some(Ok(mut door)) = door.map(|door| doors.get_mut(door)) else {
            continue;
        };
        let Some(gltf) = gltfs.get(&door.gltf) else {
            continue;
        };
        let named = |suffix: &str| {
            gltf.named_animations
                .iter()
                .find(|(name, _)| name.ends_with(suffix))
                .map(|(_, clip)| clip.clone())
        };
        let mut graph = AnimationGraph::new();
        let root = graph.root;
        if let (Some(open), Some(close)) = (named("_ACTIVATE"), named("_DEACTIVATE")) {
            door.clips = Some((
                graph.add_clip(open, 1.0, root),
                graph.add_clip(close, 1.0, root),
            ));
        }
        door.player = Some(entity);
        commands.entity(entity).insert((
            AnimationGraphHandle(graphs.add(graph)),
            AnimationTransitions::new(),
        ));
    }
}

/// Opens or closes each door that `ai` swings, playing its clip once.
fn swing(
    mut commands: Commands,
    mut doors: Query<(Entity, &mut Door, &Swing)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    mut npcs: ResMut<Npcs>,
    mut heights: ResMut<Heights>,
    speed: Res<hero::GameSpeed>,
) {
    for (entity, mut door, swing) in &mut doors {
        if !swing.open && !swing.by_hero && npcs.marked(&door) {
            commands.entity(entity).remove::<Swing>();
            continue;
        }
        let (Some((open, close)), Some(player)) = (door.clips, door.player) else {
            // Its clips not yet given, it waits, unless its model has none.
            if door.player.is_some() {
                turn_trigger(&mut npcs, &mut heights, &door, swing);
                door.open = swing.open;
                commands.entity(entity).remove::<Swing>();
            }
            continue;
        };
        if door.open != swing.open
            && let Ok((mut player, mut transitions)) = players.get_mut(player)
        {
            let clip = if swing.open { open } else { close };
            transitions
                .play(&mut player, clip, Duration::ZERO)
                .set_speed(speed.0);
        }
        turn_trigger(&mut npcs, &mut heights, &door, swing);
        door.open = swing.open;
        commands.entity(entity).remove::<Swing>();
    }
}

/// Opens or closes the cells of `door`'s trigger as `swing` turns it (bit 0 of the trigger's
/// state, docs/re/movement.md), and keeps a hero's mark on it (#372).
fn turn_trigger(npcs: &mut Npcs, heights: &mut Heights, door: &Door, swing: &Swing) {
    let Some(name) = door.trigger.as_ref() else {
        return;
    };
    if swing.by_hero {
        npcs.marked.insert(name.clone());
    }
    let spots = npcs.triggers.get(name).cloned().unwrap_or_default();
    if swing.open {
        npcs.opened.insert(name.clone());
    } else {
        npcs.opened.remove(name);
    }
    for spot in spots {
        for (cell, level) in spot_levels(heights, spot) {
            if swing.open {
                heights.free(cell, level);
            } else {
                heights.occupy(cell, level);
            }
        }
    }
}

/// An NPC that `ai` puts at a place on a level at once, as the client puts the townsfolk far from
/// the hero into a house at dusk (`0x54D9D0`).
#[derive(Component)]
pub struct PutAt {
    pub at: Vec3,
    pub level: Level,
}

/// Puts each NPC that `ai` sends somewhere at once there: its place on its level's heights, its
/// level and its layer.
fn put(
    mut commands: Commands,
    heights: Res<Heights>,
    mut npcs: Query<(
        Entity,
        &PutAt,
        &mut Transform,
        &mut crowd::InTheWay,
        &mut buildings::Layered,
    )>,
) {
    for (entity, put, mut transform, mut way, mut layered) in &mut npcs {
        let mut at = put.at;
        if let Some(y) = heights.standing(at.x, at.z, put.level) {
            at.y = y;
        }
        transform.translation = at;
        way.level = put.level;
        layered.set_if_neq(buildings::Layered::at(&heights, at.x, at.z, put.level));
        commands.entity(entity).remove::<PutAt>();
    }
}

/// An NPC that dies (the AI's state 9, `0x53E7C0`): it plays one of its dying clips, chosen by
/// `choice`, once, and lies there; it no longer thinks, walks or stands in the way.
#[derive(Component)]
pub struct Dies {
    pub choice: u32,
    played: bool,
}

impl Dies {
    pub fn new(choice: u32) -> Self {
        Self {
            choice,
            played: false,
        }
    }
}

/// Lays each dying NPC down: its dying clip once, its walk, its strike, its mind and its place in
/// the way gone.
fn die(
    mut commands: Commands,
    mut dying: Query<(Entity, &Npc, &mut Dies)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    speed: Res<hero::GameSpeed>,
) {
    for (entity, npc, mut dies) in &mut dying {
        if dies.played {
            continue;
        }
        commands
            .entity(entity)
            .remove::<(Walk, Strikes, Talks, Plays, crowd::InTheWay, ai::Mind)>();
        let (Some(graph), Some(player)) = (&npc.graph, npc.player) else {
            continue;
        };
        let Ok((mut player, mut transitions)) = players.get_mut(player) else {
            continue;
        };
        if let Some(&node) = graph
            .dying
            .get(usize::try_from(dies.choice).unwrap_or(0) % graph.dying.len().max(1))
        {
            transitions
                .play(&mut player, node, BLEND)
                .set_speed(speed.0);
        }
        dies.played = true;
    }
}

/// An NPC that strikes at `target` (the attack action 3, `0x4E4CF0`): one of its attack's clips,
/// chosen by `choice`, once, which hits at its fractions (`0x4EA1F0`); or that shoots at it (the
/// action 5, `0x4DCB80`), the clip at its attack speed times its chain's factor (`+0x210`), whose
/// arrow leaves at its first fraction (`0x4DD90E`).
#[derive(Component)]
pub struct Strikes {
    target: Entity,
    choice: u32,
    /// A shot's factor of its chain, none for a strike.
    shot: Option<f32>,
    /// The clip it plays, its length and the fractions at which it hits, once it plays.
    playing: Option<(AnimationNodeIndex, f32, [f32; 2])>,
    /// How many hits it has struck.
    struck: u8,
}

impl Strikes {
    /// A strike at `target`, with the clip `choice` of its attack's.
    pub fn at(target: Entity, choice: u32) -> Self {
        Self {
            target,
            choice,
            shot: None,
            playing: None,
            struck: 0,
        }
    }

    /// A shot at `target`, with the clip `choice` of its attack's, at its attack speed times
    /// `chain`.
    pub fn shot(target: Entity, choice: u32, chain: f32) -> Self {
        Self {
            shot: Some(chain),
            ..Self::at(target, choice)
        }
    }
}

/// An NPC whose strike, not a shot, has just played its clip whole: its mind decides whether the
/// next one chains (`ai::pursue`, `opensacred_rules::minds::Mind::ends_strike`, `0x4E4CF0`).
#[derive(Component)]
pub struct StrikeEnded;

/// How far a strike reaches when it hits, times the reach (`0x4EA1F0`; **hypothesis** that the base
/// is the reach of the fight's band).
const HITS_WITHIN: f32 = 3.0 * 130.0;

/// Plays each striking NPC's attack once, and at the clip's fractions (0.5 for a first of 0; a
/// second when it is above 0) sends a hit of its share when the target is then within reach; a
/// walk ends the strike (`0x4E4CF0`, `0x4EA1F0`). A shot plays its clip at its attack speed
/// times its chain, and at the clip's first fraction looses its arrow (`arrows::Loose`,
/// `0x4DD90E`).
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn strike(
    mut commands: Commands,
    mut strikers: Query<
        (
            Entity,
            &Npc,
            &mut Strikes,
            Has<Walk>,
            &GlobalTransform,
            Option<&fight::Combat>,
        ),
        Without<Dies>,
    >,
    places: Query<&GlobalTransform>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    clips: Res<Assets<AnimationClip>>,
    speed: Res<hero::GameSpeed>,
    mut hits: MessageWriter<fight::Hit>,
    mut shots: MessageWriter<arrows::Loose>,
) {
    for (entity, npc, mut strikes, walking, at, combat) in &mut strikers {
        let (Some(graph), Some(player)) = (&npc.graph, npc.player) else {
            continue;
        };
        let Ok((mut player, mut transitions)) = players.get_mut(player) else {
            continue;
        };
        if walking || graph.attacks.is_empty() {
            commands.entity(entity).remove::<Strikes>();
            continue;
        }
        let Some((node, length, fractions)) = strikes.playing else {
            let index = usize::try_from(strikes.choice).unwrap_or(0) % graph.attacks.len();
            let (node, clip, fractions) = &graph.attacks[index];
            let length = clips.get(clip).map_or(1.0, AnimationClip::duration);
            // At its attack speed (`0x4E4CF0`), a shot's times its chain (`0x4DCB80`).
            let attack = combat.map_or(1.0, |combat| combat.clip_speed());
            let chain = strikes.shot.unwrap_or(1.0);
            transitions
                .play(&mut player, *node, BLEND)
                .set_speed(speed.0 * attack * chain);
            strikes.playing = Some((*node, length.max(0.01), *fractions));
            continue;
        };
        let Some(active) = player.animation(node) else {
            commands.entity(entity).remove::<Strikes>();
            continue;
        };
        let done = active.seek_time() / length;
        let first = if fractions[0] > 0.0 {
            fractions[0]
        } else {
            0.5
        };
        let due = match (strikes.shot, strikes.struck) {
            (Some(_), 0) => opensacred_rules::minds::looses(fractions[0], done),
            (None, 0) => done >= first,
            (None, 1) => fractions[1] > 0.0 && done >= fractions[1],
            _ => false,
        };
        if due && strikes.shot.is_some() {
            strikes.struck += 1;
            shots.write(arrows::Loose {
                shooter: entity,
                target: strikes.target,
            });
        } else if due {
            strikes.struck += 1;
            let reaches = places.get(strikes.target).is_ok_and(|target| {
                (target.translation() - at.translation())
                    .with_y(0.0)
                    .length()
                    <= HITS_WITHIN
            });
            if reaches {
                hits.write(fight::Hit {
                    attacker: entity,
                    target: strikes.target,
                    share: npc.share,
                    ranged: false,
                    art: None,
                });
            }
        }
        if active.is_finished() {
            let mut ended = commands.entity(entity);
            ended.remove::<Strikes>();
            if strikes.shot.is_none() {
                ended.insert(StrikeEnded);
            }
        }
    }
}

/// What an NPC fights with: its type's values and its model, by which its values and its clips
/// are made anew when its weapons change ([`rearm`]), and the items in its hands with their
/// values, made at its level (`0x5D5080`).
#[derive(Component, Clone)]
pub struct Arms {
    base: fight::Base,
    /// Whether the object maker alone made it (`Placement::plain`).
    plain: bool,
    model: String,
    holding: Vec<(InHand, Option<fight::ItemValues>)>,
}

impl Arms {
    /// Its attack, defence, damage and resistances at level `grade` and of rank `rank`
    /// (`0x5796A0`), with its skills and its items' values: unarmed when what it holds is of
    /// class 0 and its right hand empty (`0x546F80`); and its hits' share of a strike, halved with
    /// two weapons, the class 8 or one in each hand, the left no shield (`0x4EA3F0`).
    fn fight(&self, grade: u32, rank: fight::Rank, rules: &fight::Rules) -> (fight::Combat, f32) {
        let in_hands: Vec<InHand> = self.holding.iter().map(|&(held, _)| held).collect();
        let class = class_of(&in_hands);
        let holds = |hand, shield: Option<bool>| {
            in_hands.iter().any(|held| {
                held.hand == hand && shield.is_none_or(|s| (held.category == SHIELD) == s)
            })
        };
        let right = holds(Hand::Right, None);
        let two = class == 8 || (right && holds(Hand::Left, Some(false)));
        let held = fight::Holds {
            weapon: class != 0 || right,
            shield: holds(Hand::Left, Some(true)),
            ranged: ranged_of(&in_hands),
            items: self
                .holding
                .iter()
                .filter_map(|&(_, values)| values)
                .collect(),
        };
        let skills = fight::skills(&self.base, grade, self.plain);
        let mut values = fight::combat(&self.base, grade, held, &skills, rules);
        let [attack, defence] = rank.fight(rules);
        values.attack *= attack;
        values.defence *= defence;
        (fight::Combat(values), if two { 0.5 } else { 1.0 })
    }
}

/// An NPC that changes its weapons (#396; docs/re/fight.md, "The archers"): it puts away what
/// its hand `away` holds, and takes the item `takes`, if any, in its other hand in place of what
/// that holds.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rearms {
    away: Hand,
    takes: Option<u32>,
}

impl Rearms {
    /// An archer's switch to melee (`0x4D4B70`): the bow of its left hand away (slot 12), and
    /// `weapon`, if any, in its right (slot 13).
    pub fn to_melee(weapon: Option<u32>) -> Self {
        Self {
            away: Hand::Left,
            takes: weapon,
        }
    }

    /// The bow-taking (`0x4D4A80`): its right hand emptied (slot 13), and `bow` in its left
    /// (slot 12).
    pub fn bow(bow: u32) -> Self {
        Self {
            away: Hand::Right,
            takes: Some(bow),
        }
    }

    /// The hand that takes the new item.
    fn other(self) -> Hand {
        match self.away {
            Hand::Left => Hand::Right,
            Hand::Right => Hand::Left,
        }
    }
}

/// Changes the weapons of each NPC that rearms ([`Rearms`]): the items it puts away leave its
/// hands and their models go, the new item, made at its level (`0x5D5080`), goes into its hand
/// with its model, and its attack, defence, damage and resistances, its hits' share, its clips by
/// the class of what it holds (`motions::Slots::stance`, `attacks`) and whether it fights at range
/// are made anew.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn rearm(
    mut commands: Commands,
    npcs: Res<Npcs>,
    assets: Res<AssetServer>,
    mut armed: Query<(
        Entity,
        &Rearms,
        &mut Arms,
        &mut Npc,
        &mut ai::Mind,
        &mut fight::Combat,
        &fight::Ranked,
        Option<&mut Equipment>,
        &Transform,
    )>,
    wielded: Query<(Entity, &Wielded, &models::OrderedWith)>,
) {
    for (entity, rearms, mut arms, mut npc, mut mind, mut combat, rank, equipment, transform) in
        &mut armed
    {
        commands.entity(entity).remove::<Rearms>();
        let other = rearms.other();
        let gone = |hand: Hand| hand == rearms.away || (rearms.takes.is_some() && hand == other);
        let put_away: Vec<u32> = arms
            .holding
            .iter()
            .filter(|(held, _)| gone(held.hand))
            .map(|(held, _)| held.item)
            .collect();
        arms.holding.retain(|(held, _)| !gone(held.hand));
        for (model, held, _) in wielded.iter().filter(|(_, _, by)| by.0 == entity) {
            if gone(held.0) {
                commands.entity(model).despawn();
            }
        }
        let mut equipment = equipment;
        if let Some(equipment) = equipment.as_mut() {
            equipment.0.retain(|item| !gone(item.hand));
        }
        let grade = mind.level;
        if let Some(item) = rearms.takes
            && let Some(gear) = npcs.gear.get(&item)
        {
            let (x, y) = paths::cell_of(transform.translation.x, transform.translation.z);
            let mut dice = spawning::Rng::of(
                u32::try_from(x).unwrap_or(0),
                u32::try_from(y).unwrap_or(0),
                usize::try_from(item).unwrap_or(0),
            );
            let values = npcs.items.make(item, grade, &mut dice);
            let held = InHand {
                item,
                category: gear.category,
                kind: gear.kind,
                hand: other,
            };
            arms.holding.push((held, values));
            if let Some(model) = &gear.model {
                let equip = Equip {
                    gltf: assets.load(model.clone()),
                    texture: gear.texture.as_ref().map(|t| load_texture(&assets, t)),
                    hand: other,
                    shield: gear.category == SHIELD,
                    ready: true,
                    waits: 0,
                };
                match equipment {
                    Some(mut equipment) => equipment.0.push(equip),
                    None => {
                        commands.entity(entity).insert(Equipment(vec![equip]));
                    }
                }
            }
        }
        let (values, share) = arms.fight(grade, rank.0, &npcs.fight);
        // Its pace by what it holds now, and its walk and run by it.
        let pace = values.0.pace_factor();
        let ratio = pace / npc.pace.max(f32::EPSILON);
        mind.speed *= ratio;
        mind.run *= ratio;
        npc.pace = pace;
        *combat = values;
        npc.share = share;
        let in_hands: Vec<InHand> = arms.holding.iter().map(|&(held, _)| held).collect();
        npc.hands_empty = in_hands.is_empty();
        mind.ranged = ranged_of(&in_hands);
        // Its clips by what it holds now: its graph is made anew (`animate`).
        if let Some(slots) = npcs.slots.get(&arms.model) {
            npc.names = ClipNames::of(slots, class_of(&in_hands));
            npc.graph = None;
            if let Some(player) = npc.player.take() {
                commands
                    .entity(player)
                    .remove::<(AnimationGraphHandle, AnimationTransitions)>();
            }
        }
        bevy::log::debug!(
            "npcs: {entity} of {} puts away {put_away:?} and holds {:?}, of class {}, at range \
             {}, its attack {:.0}",
            arms.base.id,
            in_hands.iter().map(|held| held.item).collect::<Vec<_>>(),
            class_of(&in_hands),
            mind.ranged,
            values.attack,
        );
    }
}

/// A juggler that plays its instrument where it stands (`0x4F29E2`): `instrument` the one it
/// takes into its right hand when both its hands are empty, of [`INSTRUMENTS`]; `on` whether it
/// plays on, one of the flag `0x10000000`.
#[derive(Component)]
pub struct Plays {
    pub instrument: usize,
    pub on: bool,
}

/// Gives a juggler about to play its instrument, its hands empty, the instrument in its right hand.
fn instrument(
    mut commands: Commands,
    npcs: Res<Npcs>,
    assets: Res<AssetServer>,
    mut jugglers: Query<(Entity, &mut Npc, &Plays), Without<Walk>>,
) {
    for (entity, mut npc, plays) in &mut jugglers {
        if !npc.hands_empty || npc.instrument.is_some() || npc.player.is_none() {
            continue;
        }
        let Some(Some((model, texture))) = npcs.instruments.get(plays.instrument) else {
            continue;
        };
        npc.hands_empty = false;
        npc.instrument = Some(plays.instrument);
        commands.entity(entity).insert(Equipment(vec![Equip {
            gltf: assets.load(model.clone()),
            texture: texture.as_ref().map(|t| load_texture(&assets, t)),
            hand: Hand::Right,
            shield: false,
            ready: true,
            waits: 0,
        }]));
    }
}

/// An NPC that plays its talk once where it stands (`*_TALK_A`, slot 177; `ai`'s meetings).
#[derive(Component)]
pub struct Talks;

/// The NPCs and objects that the scripts and the fills place: placed as their sectors come in,
/// animated, walked, retextured and equipped.
pub struct NpcsPlugin;

impl Plugin for NpcsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Retextured>()
            .add_observer(retexture)
            .add_observer(equipment_ready)
            .add_observer(keep)
            .add_systems(
                Update,
                (
                    place, rearm, animate, door_clips, walk, put, swing, instrument, strike, die,
                    stance, equip,
                )
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
    fn a_bow_a_crossbow_or_a_musket_in_a_hand_is_a_ranged_weapon() {
        assert!(ranged(&[(WEAPON, Some(BOW))]));
        assert!(ranged(&[(WEAPON, Some(1)), (WEAPON, Some(CROSSBOW))]));
        assert!(ranged(&[(WEAPON, Some(14))]));
        // A sword and a shield; a bow whose hand a first one took; arrows are no weapon.
        assert!(!ranged(&[(WEAPON, Some(1)), (SHIELD, Some(0))]));
        assert!(!ranged(&[(SHIELD, None), (WEAPON, Some(BOW))]));
        assert!(!ranged(&[(27, Some(BOW))]));
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
    fn an_objects_model_shows_where_its_world_position_does() {
        use opensacred_data::world::{Cell, Sector};
        let op = |tag, value| Operand { tag, value };
        // A door of cell (10, 20) whose world position lies three cells along x off its centre.
        let ops = [
            op(0x02, Value::I32(5650)),
            op(0x04, Value::I32x3([10, 20, 0])),
            op(WORLD_POSITION, Value::I32x3([724, 1100, 0])),
            op(TRIGGER, Value::Str("door_1".into())),
        ];
        let drawn = drawn_at(&ops);
        assert_eq!(drawn, Some(Vec2::new(724.0, 1100.0)));
        assert_eq!(drawn_at(&ops[..2]), None);
        // The ground a creature stands on rises from the cells' column 12 on.
        let cell = |raised: bool| Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [if raised { 8 } else { 0 }; 4],
            offsets: [0; 2],
            flags: 0,
            ground: 0x40,
        };
        let mut heights = Heights::default();
        heights.add(&Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells: (0..SIDE * SIDE).map(|i| cell(i % SIDE >= 12)).collect(),
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: Vec::new(),
        });
        let centre = iso::ground(10.5, 20.5);
        let at = centre.with_y(heights.standing(centre.x, centre.z, None).unwrap());
        // Where the screen shows its world position at height 0, at its cell's height, its cell's
        // centre staying its logical position: the ground raised at the world position is not
        // read, as the client reads none there.
        let close = |a: Vec3, b: Vec3| (a - b).length() < 1e-2;
        assert!(heights.standing(724.0, 1100.0, None).unwrap() > at.y);
        assert_eq!(at.y, 0.0);
        assert!(close(
            model_position(at, drawn),
            Vec3::new(724.0, 0.0, 1100.0)
        ));
        // A door on a building's upper storey, 263 units up, whose world position lies on the
        // ground in front of the building, as the editor took it: its leaf stands on the floor,
        // √(3/2) units along X and Z for a unit of the floor's height, the client's 1.92 world
        // units for a unit of the models' height (`0x485C10`).
        let floor = at.with_y(263.0);
        let along = 263.0 * 1.5_f32.sqrt();
        assert!(close(
            model_position(floor, Some(Vec2::new(300.0, 400.0))),
            Vec3::new(300.0 + along, 263.0, 400.0 + along)
        ));
        // Without a world position at its cell's centre.
        assert_eq!(model_position(at, None), at);
        assert_eq!(model_position(floor, None), floor);
    }

    #[test]
    fn a_set_npc_state_gives_the_creature_it_names_its_level_or_makes_it_a_boss() {
        let op = |tag, value| Operand { tag, value };
        let named = |kind, name: &str, rest: &[Operand]| {
            let mut ops = vec![op(0x01, Value::Str(name.into()))];
            ops.extend_from_slice(rest);
            Record {
                kind,
                body: Body::Operands(ops),
            }
        };
        let level = |n| op(LEVEL, Value::I32(n));
        let trader = op(0x63, Value::Str("res:17321".into()));
        // The stable of 64,41: the trader, a horse whose `SetNPCState` gives it 13 in place of
        // the level its `CreateNPC` gave it (a `SetNPCState` before it names none yet), and one
        // whose `SetNPCState` gives none.
        let records = [
            named(CREATE_NPC, "res:17321", &[op(0x02, Value::I32(656))]),
            named(SET_NPC_STATE, "res:17421", &[level(7), trader.clone()]),
            named(
                CREATE_NPC,
                "Res:17421",
                &[op(0x02, Value::I32(551)), op(0x90, Value::I32(4))],
            ),
            named(SET_NPC_STATE, "res:17421", &[level(13), trader.clone()]),
            named(CREATE_NPC, "res:17424", &[op(0x02, Value::I32(551))]),
            named(SET_NPC_STATE, "res:17424", &[trader]),
            // A boss of a quest's, and a creature that made no placement.
            named(CREATE_NPC, "res:17190", &[op(0x02, Value::I32(76))]),
            named(
                SET_NPC_STATE,
                "res:17190",
                &[op(0x08, Value::Marker), level(1000)],
            ),
            named(CREATE_NPC, "res:17999", &[op(0x02, Value::I32(76))]),
            named(SET_NPC_STATE, "res:17999", &[level(9)]),
        ];
        let records: Vec<&Record> = records.iter().collect();
        let made: HashMap<*const Record, usize> = [0, 2, 4, 6]
            .iter()
            .enumerate()
            .map(|(index, &r)| (std::ptr::from_ref(records[r]), index))
            .collect();
        let mut placements = vec![Placement::default(); 4];
        placements[1].tag = 4;
        set_states(&records, &made, &mut placements);
        let levels: Vec<(u32, fight::Rank)> = placements.iter().map(|p| (p.tag, p.rank)).collect();
        assert_eq!(
            levels,
            [
                (0, fight::Rank::Common),
                (13, fight::Rank::Common),
                (0, fight::Rank::Common),
                (1000, fight::Rank::Boss),
            ]
        );
    }

    #[test]
    fn a_set_npc_state_gives_the_creature_its_dialogue_side_flags_home_and_trade() {
        let op = |tag, value| Operand { tag, value };
        let marker = |tag| op(tag, Value::Marker);
        let named = |kind, name: &str, rest: &[Operand]| {
            let mut ops = vec![op(0x01, Value::Str(name.into()))];
            ops.extend_from_slice(rest);
            Record {
                kind,
                body: Body::Operands(ops),
            }
        };
        let dialog = |name: &str| Record {
            kind: 40,
            body: Body::Dialog {
                name: name.into(),
                values: [0, 0],
            },
        };
        let talk = |name: &str| op(DIALOGUE, Value::Str(name.into()));
        let records = [
            // The captain of 52,39 (`0E 46 13`), then the table's first entry, twice in it.
            named(
                CREATE_NPC,
                "Res:17085",
                &[marker(0x0E), marker(0x46), marker(0x13)],
            ),
            dialog("MP_Romata_99"),
            // A dialogue not in the table yet takes none; the first entry's index 0 none.
            named(SET_NPC_STATE, "res:17085", &[talk("auftrag22")]),
            named(SET_NPC_STATE, "res:17085", &[talk("mp_romata_99")]),
            dialog("auftrag22"),
            named(SET_NPC_STATE, "res:17085", &[talk("AUFTRAG22")]),
            // A townsperson made hostile with a guard's mode, sent to another home.
            named(CREATE_NPC, "res:640", &[marker(0x2B)]),
            named(
                SET_NPC_STATE,
                "res:640",
                &[
                    marker(0x08),
                    marker(0x46),
                    op(HOME, Value::I32x3([3230, 2766, 0])),
                ],
            ),
            // A trader whose mode `0x800` takes its trade away, its dialogue by number with it;
            // another made one with its dialogue stamped.
            named(CREATE_NPC, "res:656", &[marker(0x24), talk("auftrag22")]),
            named(
                SET_NPC_STATE,
                "res:656",
                &[marker(0x43), op(DIALOGUE_NUMBER, Value::I32(0))],
            ),
            named(CREATE_NPC, "res:657", &[marker(0x42)]),
            named(SET_NPC_STATE, "res:657", &[marker(0x64)]),
        ];
        let records: Vec<&Record> = records.iter().collect();
        let made: HashMap<*const Record, usize> = [0, 6, 8, 10]
            .iter()
            .enumerate()
            .map(|(index, &r)| (std::ptr::from_ref(records[r]), index))
            .collect();
        let mut placements = vec![Placement::default(); 4];
        placements[0].creature = (297, 0, ways::flags_word([0x0E, 0x46, 0x13]));
        placements[0].side = 7;
        placements[1].creature = (640, 0, ways::flags_word([0x2B]));
        placements[1].side = 3;
        placements[2].trader = true;
        placements[2].dialogue = true;
        placements[2].creature.2 = ways::flags_word([0x24]);
        placements[3].creature.2 = ways::flags_word([0x42]);
        // The captain's dialogue at each `SetNPCState`, the last its own.
        let mut captain = placements[0].clone();
        let state = |ops: &Record, table: &[&str]| {
            let Body::Operands(ops) = &ops.body else {
                unreachable!("a SetNPCState has operands");
            };
            let table: Vec<String> = table.iter().map(|&d| d.to_owned()).collect();
            State::of(ops, &table)
        };
        state(records[2], &["MP_Romata_99", "MP_Romata_99"]).apply(&mut captain);
        assert!(!captain.dialogue);
        captain.dialogue = true;
        state(records[3], &["MP_Romata_99", "MP_Romata_99"]).apply(&mut captain);
        assert!(!captain.dialogue);
        set_states(&records, &made, &mut placements);
        assert!(placements[0].dialogue);
        assert_eq!(placements[0].side, 7);
        assert_eq!(placements[0].creature.2, 0x2);
        // The townsperson on the side 14, a guard, at home at 3230,2766.
        assert_eq!(placements[1].side, 14);
        assert_eq!(placements[1].creature.2, 0x4000);
        assert_eq!(placements[1].home, Some((3230, 2766)));
        // The trader no more, its mode as it was, its dialogue gone; the other a trader that
        // talks.
        assert!(!placements[2].trader);
        assert!(!placements[2].dialogue);
        assert_eq!(placements[2].creature.2, ways::flags_word([0x24]));
        assert!(placements[3].trader);
        assert!(placements[3].dialogue);
        assert_eq!(placements[3].creature.2, 0x400);
    }

    #[test]
    fn a_create_npc_takes_the_last_of_its_level_tags() {
        let op = |tag, value| Operand { tag, value };
        let creature = op(0x02, Value::I32(32));
        let tagged = |tags: &[Value]| {
            let mut ops = vec![creature.clone()];
            ops.extend(tags.iter().map(|value| op(LEVEL, value.clone())));
            ops.push(op(0x08, Value::Marker));
            level_tag(&ops)
        };
        assert_eq!(tagged(&[]), 0);
        assert_eq!(tagged(&[Value::I32(5)]), 5);
        // The bosses of a fight that names their level twice (`bosskampf1_9011`).
        assert_eq!(tagged(&[Value::I32(1000), Value::I32(1000)]), 1000);
        assert_eq!(tagged(&[Value::I32(1000), Value::I32(6)]), 6);
        assert_eq!(tagged(&[Value::I32(-3)]), 0);
        assert_eq!(
            tagged(&[Value::I32(5), Value::Variable("dq_num".into())]),
            0
        );
        assert_eq!(
            fight::Rank::of_tag(tagged(&[Value::I32(1000)])),
            fight::Rank::Boss
        );
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
    fn the_creatures_that_went_with_their_sectors_come_back_as_they_were() {
        let health = |current| fight::Health {
            current,
            most: 72,
            level: 3,
        };
        let stood = |x, current| Stood {
            cell: (x, 2518),
            layer: 0,
            health: health(current),
        };
        let mut kept = Kept::default();
        // A kept placement: anew at first, not again while it lives somewhere, where it stood
        // once it went living, never once killed.
        let goblin = Placed {
            at: (51, 39),
            source: Source::Kept(7),
        };
        assert_eq!(kept.again(7), Again::Anew);
        kept.spawned(&goblin);
        assert_eq!(kept.again(7), Again::Not);
        kept.gone(&goblin, Some(stood(3300, 40)));
        assert_eq!(kept.again(7), Again::AtStood(stood(3300, 40)));
        kept.spawned(&goblin);
        kept.gone(&goblin, None);
        assert_eq!(kept.again(7), Again::Not);
        // A sector's fills: anew at first; back the living ones that went, and not anew while
        // one lives somewhere still; anew once none lives.
        let at = (49, 39);
        let rabbit = || Placed {
            at,
            source: Source::Filled(Box::new(Placement {
                model: "rabbit".to_owned(),
                ..Placement::default()
            })),
        };
        assert!(kept.back(at).1);
        let (first, second) = (rabbit(), rabbit());
        kept.spawned(&first);
        kept.spawned(&second);
        kept.gone(&first, Some(stood(3150, 72)));
        let (back, anew) = kept.back(at);
        assert_eq!((back.len(), anew), (1, false));
        assert_eq!(back[0].0.model, "rabbit");
        assert_eq!(back[0].1, stood(3150, 72));
        // The other still lives somewhere: nothing back, and no fill anew.
        let (back, anew) = kept.back(at);
        assert!(back.is_empty() && !anew);
        kept.gone(&second, None);
        assert!(kept.back(at).1);
    }

    #[test]
    fn a_walk_shows_its_clip_once_it_has_stepped() {
        let node = AnimationNodeIndex::new;
        let clips = Clips {
            graph: Handle::default(),
            idle: node(0),
            walk: Some(node(1)),
            run: Some(node(2)),
            stance: None,
            talk: None,
            fun: [None; 4],
            attacks: Vec::new(),
            dying: Vec::new(),
        };
        let mut walk = Walk::to(Vec3::ZERO, None, 70.0);
        // Not yet stepped, as a walk whose first step is blocked: it stands in its idle.
        assert_eq!(clips.moving(&walk), None);
        walk.moved = true;
        assert_eq!(clips.moving(&walk), Some(node(1)));
        let mut run = Walk::run(Vec3::ZERO, None, 140.0);
        run.moved = true;
        assert_eq!(clips.moving(&run), Some(node(2)));
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
    fn an_archer_that_rearms_holds_its_new_weapon_in_the_other_hand() {
        // A bow and a sword, a ring between: the bow in the left hand, the sword in the right.
        let holding = InHand::of([
            (1733, WEAPON, Some(BOW)),
            (900, 8, None),
            (1753, WEAPON, Some(1)),
        ]);
        let hands: Vec<(u32, Hand)> = holding.iter().map(|h| (h.item, h.hand)).collect();
        assert_eq!(hands, [(1733, Hand::Left), (1753, Hand::Right)]);
        assert_eq!(
            class_of(&holding),
            held_class(&[(WEAPON, Some(BOW)), (WEAPON, Some(1))])
        );
        assert!(ranged_of(&holding));
        // The switch to melee puts away the left hand's bow for the right's new weapon; the
        // bow-taking the right hand's weapon for the left's bow.
        let to_melee = Rearms::to_melee(Some(1753));
        assert_eq!((to_melee.away, to_melee.other()), (Hand::Left, Hand::Right));
        let bow = Rearms::bow(1732);
        assert_eq!((bow.away, bow.other()), (Hand::Right, Hand::Left));
        let sword = InHand::of([(1753, WEAPON, Some(1))]);
        assert_eq!(class_of(&sword), 1);
        assert!(!ranged_of(&sword));
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
    fn the_quests_the_start_sets_up_place_their_givers_after_their_set_up() {
        let op = |tag, value| Operand { tag, value };
        let record = |kind, ops| Record {
            kind,
            body: Body::Operands(ops),
        };
        let quest = |kind, n| record(kind, vec![op(0x0B, Value::I32(n))]);
        let function = |name: &str, records| Function {
            name: name.into(),
            quest: 15004,
            region: 0,
            records,
        };
        // The giver of quest 15004 by the garrison of 52,39 (#624): peaceful, staying, with its
        // dialogue.
        let giver = |n| {
            record(
                CREATE_NPC,
                vec![
                    op(0x01, Value::Str(format!("res:AUFTRAGGEBER_{n}"))),
                    op(0x02, Value::I32(288)),
                    op(0x04, Value::Name(format!("POS_AUFTRAG_{n}"))),
                    op(0x0E, Value::Marker),
                    op(0x74, Value::Marker),
                    op(0x09, Value::Str(format!("DLG_{n}_START"))),
                ],
            )
        };
        let defpos = record(
            DEF_POS,
            vec![
                op(0x01, Value::Str("POS_AUFTRAG_15004".into())),
                op(0x0B, Value::I32(3370)),
                op(0x0B, Value::I32(2524)),
            ],
        );
        let tree = Tree {
            // Quest 15004 set up twice, quest 7 set up without an `OnSetUp`; quest 15018's
            // `OnSetUp` is never run, its quest not set up.
            start: vec![
                defpos,
                quest(SET_UP_QUEST, 15004),
                quest(SET_UP_QUEST, 7),
                quest(SET_UP_QUEST, 15004),
            ],
            quest: Vec::new(),
            functions: vec![
                function("QIS_OnSetUp15004", vec![giver(15004)]),
                function("QIS_OnSetUp15018", vec![giver(15018)]),
            ],
            quests: Vec::new(),
            regions: Vec::new(),
        };
        let records = start_records(&tree);
        let kinds: Vec<u16> = records.iter().map(|r| r.kind).collect();
        assert_eq!(
            kinds,
            [
                DEF_POS,
                SET_UP_QUEST,
                CREATE_NPC,
                SET_UP_QUEST,
                SET_UP_QUEST
            ]
        );
        let Body::Operands(ops) = &records[2].body else {
            panic!("the giver has operands");
        };
        assert_eq!(name_of(ops).as_deref(), Some("res:auftraggeber_15004"));
        let named = positions(records.iter().copied());
        assert_eq!(position(ops, &named), Some((3370, 2524, 0)));
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
        let chest = |x, y, floor| ("chesttrigger".to_owned(), (x, y, floor));
        assert_eq!(
            closed_cells(&records),
            [
                chest(3341, 2512, 1),
                chest(3342, 2512, 1),
                chest(3343, 2512, 1),
                chest(3200, 2490, 0)
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

    #[test]
    fn every_sector_keeps_its_values_with_fills_or_without() {
        let op = |tag, value| Operand { tag, value };
        let record = |kind, ops| Record {
            kind,
            body: Body::Operands(ops),
        };
        let spawn_values = |lowest| {
            let numbers = [50, lowest, lowest + 3];
            record(
                SPAWN_VALUES,
                numbers.map(|n| op(0x0B, Value::I32(n))).to_vec(),
            )
        };
        let fill = record(
            FILL_SECTOR,
            vec![
                op(0x35, Value::I32x3([4, 4, 4096])),
                op(0x19, Value::I32x3([568, 25, 1])),
            ],
        );
        let function = |name: &str, records| Function {
            name: name.into(),
            quest: -1,
            region: 0,
            records,
        };
        // The wolves' sector by the blacksmith's, whose plateau has no fill (#530).
        let tree = Tree {
            start: Vec::new(),
            quest: Vec::new(),
            functions: vec![
                function("Sector49044Init", vec![spawn_values(4)]),
                function("Sector49044Enter", vec![spawn_values(4), fill]),
                function("Sector50044Init", vec![spawn_values(4)]),
                function("Sector50044Enter", vec![spawn_values(4)]),
                function("Sector52044Enter", vec![spawn_values(1)]),
            ],
            quests: Vec::new(),
            regions: Vec::new(),
        };
        let Sectors { values, fills } = sectors(&tree);
        let levels = |at| values.get(&at).map(|v| (v.lowest, v.highest));
        assert_eq!(levels((49, 44)), Some((4, 7)));
        assert_eq!(levels((50, 44)), Some((4, 7)));
        assert_eq!(levels((52, 44)), Some((1, 4)));
        assert_eq!(levels((51, 44)), None);
        assert_eq!(fills.keys().collect::<Vec<_>>(), [&(49, 44)]);
    }

    #[test]
    fn the_spawned_creatures_take_their_items_by_the_first_rule_that_names_them() {
        let op = |tag, value| Operand { tag, value };
        let record = |kind, ops| Record {
            kind,
            body: Body::Operands(ops),
        };
        // Pool 125 of two bows, the second twice; pool 134 of one sword; a shield's pool 50.
        let pool = |pool, item| {
            record(
                WAFFENPOOL,
                vec![op(0x0B, Value::I32(pool)), op(0x02, Value::I32(item))],
            )
        };
        let records = [
            pool(125, 1811),
            pool(125, 1812),
            pool(125, 1812),
            pool(134, 1729),
            pool(50, 1203),
        ];
        let records: Vec<&Record> = records.iter().collect();
        let pools = pools(&records);
        assert_eq!(pools[&125], [1811, 1812, 1812]);
        // Thieves: always a bow from 125, a shield never after a bow; a sword's wielder 304
        // takes the shield by its chance.
        let rule = |types: Vec<u32>, items| Rule { types, items };
        let rules = [
            rule(vec![306, 313], [(125, 100), (50, 100), (-1, 0)]),
            rule(vec![304], [(134, 100), (50, 100), (-1, 0)]),
            rule(vec![306], [(134, 100), (0, 0), (-1, 0)]),
        ];
        let gear = |category, kind| Gear {
            category,
            kind,
            model: Some("m.glb".into()),
            texture: None,
            counts: true,
        };
        let gear: HashMap<u32, Gear> = [
            (1811, gear(WEAPON, Some(BOW))),
            (1812, gear(WEAPON, Some(BOW))),
            (1729, gear(WEAPON, Some(1))),
            (1203, gear(SHIELD, None)),
        ]
        .into();
        let mut rng = spawning::Rng::of(48, 39, 0);
        let kinds = |items: Vec<(u32, &Gear)>| {
            items
                .iter()
                .map(|(_, g)| (g.category, g.kind))
                .collect::<Vec<_>>()
        };
        // The first rule that names 306 wins: a bow alone.
        assert_eq!(
            kinds(gear_of(&rules, 306, &pools, &gear, &mut rng)),
            [(WEAPON, Some(BOW))]
        );
        assert_eq!(
            kinds(gear_of(&rules, 304, &pools, &gear, &mut rng)),
            [(WEAPON, Some(1)), (SHIELD, None)]
        );
        // A type no rule names takes nothing.
        assert!(gear_of(&rules, 516, &pools, &gear, &mut rng).is_empty());
    }
}
