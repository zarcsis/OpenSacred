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
//! model has no walk, then stands (docs/decisions/netscript.md, "NPCs sent by scripts walk the
//! hero's way").
//!
//! What the operands mean is read off the data (**hypothesis**, docs/02 §7.3): the first `0x02`
//! is the creature, which is also the `items.pak` slot of its model (`SOLDIER.GRN`, `HORSE.GRN`),
//! the further `0x02` its equipment (swords, bows, shields); `0x04` its position, the
//! name of a `DefPos` record (whose two `0x0B` give the global cell) or a global cell `(x, y, 0)`;
//! `0x03` a heading in degrees, an angle on the screen that the client takes into the grid's axes
//! (`0x44DDC0`, docs/02 §7.3, `heading`). A `CreateObj` record has the same `0x02`, `0x04` and
//! `0x03`, and names its trigger and its contents in `0x29` and `0x41` (**hypothesis**, by the
//! same reading); one of `0x8E` lays a quest's item, which the hero picks up and whose `0x41`
//! function its taking runs ([`QuestItem`], #1256), named by its `res:` name's text where it has
//! one ([`TextName`], #1360); an object without a heading above 0 faces its item's own angle
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
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::ecs::query::Or;
use bevy::gltf::{Gltf, GltfNode};
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::math::{Affine3A, Quat, Vec3Swizzles};
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::{
    AnimationClip, AnimationGraph, AnimationGraphHandle, AnimationNodeIndex, AnimationPlayer,
    AnimationTransitions, App, AssetId, AssetServer, Assets, ChildOf, Children, Commands,
    Component, DetectChangesMut, Entity, GlobalTransform, GltfAssetLabel, Handle, Has, Image, In,
    IntoScheduleConfigs, MeshMaterial3d, MessageReader, MessageWriter, Name, On, Plugin, Query,
    Remove, Res, ResMut, Resource, StandardMaterial, Time, Transform, Update, Vec2, Vec3, With,
    Without, WorldAssetRoot,
};
use bevy::world_serialization::{WorldAsset, WorldInstanceReady};
use opensacred_data::heroes::Worn;
use opensacred_data::netscript::{Body, Record, Tree, Value};
use opensacred_data::tables::{Creature, Item};
use opensacred_data::world::SIDE;
use opensacred_rules::fight::Effect;
use opensacred_rules::quests::DeathFunctions;
use opensacred_rules::{minds, quests, ways};

use crate::files;
use crate::frame::{Step, WorldTick};
use crate::sectors::Streamer;
use crate::tables::Tables;
use crate::terrain::Heights;
use crate::terrain::Level;
use crate::{
    ai, arrows, buildings, crowd, daynight, drops, fight, hero, iso, models, motions, paths,
    sectors, sfx, shadows, spawning,
};

/// Record types of the scripts (`opensacred_formats::netscript::KEYWORDS`).
const CREATE_NPC: u16 = 1;
const CREATE_OBJ: u16 = 8;
const SET_NPC_STATE: u16 = 3;
const SET_HP: u16 = 52;
const DEL_NPC: u16 = 55;
const SET_GROUP_STATE: u16 = 73;
const SET_ICON: u16 = 86;
const SET_RGN_DIALOG: u16 = 106;
const TELEPORT: u16 = 46;
const REGION_CHANGE: u16 = 116;
/// The prefix of a position that names a creature's place (`CPOS:HERO`, `CPOS:res:17190`).
const CREATURE_POSITION: &str = "cpos:";
/// The operand of a `SetGroupState` that removes its group's living creatures (`0x49E760`).
const REMOVES: u8 = 0x51;
/// The operand of a `CreateNPC` or a `SetNPCState` that names its dialogue, and that of a
/// `SetNPCState` that gives its number (`0x48BB40`).
const DIALOGUE: u8 = 0x09;
const DIALOGUE_NUMBER: u8 = 0x0A;
/// The operand of a `SetNPCState` that gives its creature's home, a global cell and its floor
/// (`0x461540`, `+0x2B9`).
const HOME: u8 = 0x4D;
/// The tags of `CreateNPC` that make a trader, and its bits of `+0x200`: the smiths, the
/// merchants, the wizards and one with a dialogue stamped (`0x484326`…; docs/re/ai.md, "The
/// roll").
const TRADES: [(u8, u32); 4] = [
    (0x23, quests::SMITH),
    (0x24, quests::MERCHANT),
    (0x25, quests::WIZARD),
    (0x64, quests::TRADER),
];
/// The tag of a trader whose dialogue `CreateNPC` stamps (`0x4843AF`).
const STAMPED: u8 = 0x64;
/// The tag of `CreateNPC` and `SetNPCState` whose number above 0 lets a dead follower rise
/// (`+0x2B7` bit `0x08`), and whose 0 alone lets a `CreateNPC`'s type take its area's
/// `RegionChange` list, the value −1 where the record has no such tag (`0x482F41`, #1102).
const RISES: u8 = 0x6B;
/// The operand that `scripts` adds to a `CreateNPC` of the play for each area whose
/// `RegionChange` list turns its type as it runs (#1063): the area, the new type and the pair's
/// flags (`quests::Turn`), as `I32x3`; past the scripts' last tag, `0xA0`.
pub(crate) const AREA_TURN: u8 = 0xF0;
/// The tags of `CreateNPC` that set `+0x200` bit `0x200` (`0x482510`), which keeps a creature
/// out of its first update's flags (`opensacred_rules::minds::Mind::hold`).
const KEEPS: [u8; 4] = [0x1A, 0x70, 0x71, 0x72];
/// The tag of a `CreateNPC` whose number puts its creature in that group (`+0x249`, through
/// `0x4A9EF0`, at `0x483A46`), and the bit that keeps the scripts' groups apart from the fills'
/// packs' numbers ([`script_group`]).
const GROUP: u8 = 0x11;
pub(crate) const SCRIPT_GROUP: u32 = 0x8000_0000;
/// The tags of a creature's death's function (`CreateNPC`, `SetNPCState`; #656), `0x05`, and of
/// its sight function, `0x60`, kept as `See:` and its name (#1025), which also sets the flags
/// word's bit `0x400000` (its reader not read, not followed).
const ON_DEATH: u8 = 0x05;
const ON_SIGHT: u8 = 0x60;
/// The tags of `CreateNPC` and `SetNPCState` that hire their creature as the hero's follower or
/// let it go, and the bits of their record's that each sets and clears, which add up (the reader
/// `0x48BB40`; #657): `0x100` hires, `0x200` keeps the follower fighting, `0x400` lets it go.
const HIRES: [(u8, u32, u32); 5] = [
    (0x37, 0x100, 0),
    (0x4B, 0x300, 0),
    (0x71, 0x900, 0),
    (0x39, 0x400, 0x300),
    (0x65, 0x400, 0x200),
];
/// The tag of the trader whose stock of horses a `SetNPCState`'s creature joins (#658).
const STOCK: u8 = 0x63;
/// The operand of a `CreateNPC` that gives its creature's level, or makes it a boss
/// (`0x482510`, `fight::tagged_level`).
const LEVEL: u8 = 0x1F;
const DEF_POS: u16 = 23;
const CREATE_TRIGGER: u16 = 48;
const TRIGGER_PATCH: u16 = 50;
const FILL_SECTOR: u16 = 51;
const SPAWN_VALUES: u16 = 100;
/// `Waffenpool`.
const WEAPON_POOL: u16 = 114;
const MAKE_GROUP: u16 = 115;
/// The tags of `CreateTrigger` that set and clear bit 0 of the trigger's state, its open bit, and
/// that lock it, clearing bit 0 and setting `0x8000`, and unlock it (`0x496520`).
const OPEN: u8 = 0x26;
const CLOSED: u8 = 0x27;
const LOCKED: u8 = 0x2C;
const UNLOCKED: u8 = 0x2D;
/// `CreateObj`'s operand that names its trigger.
const TRIGGER: u8 = 0x29;
/// `CreateObj`'s operand of the object's world position, `(x, y, layer)` (`0x485C10`).
const WORLD_POSITION: u8 = 0x20;
const START_POSITION: u16 = 45;
const NPC_GOTO: u16 = 72;
/// The records that turn the scripts' anim mode on and off (`SetAnimMode`, `SetCinemaMode`,
/// `SetPlayMode`; the mode word `0xAB73DC`), under which a creature's walks and teleports wait
/// their turns in its queue ([`ScriptQueue`], #1220).
const SET_ANIM_MODE: u16 = 88;
const SET_PLAY_MODE: u16 = 89;
const SET_CINEMA_MODE: u16 = 134;
/// `PlayAnim` (`0x4A2550`): a creature plays a clip of its model's slot (#1277).
const PLAY_ANIM: u16 = 91;
/// The other records that wait their turn in a creature's queue under the anim mode, and the
/// rank of the `0x01` that names the creature: `Wait`, `Attack`, `ActObj` the first, a
/// `CallFunktion` the second (docs/re/netscript.md, "The other kinds, the waits and the cinema").
const QUEUED_OTHERS: [(u16, usize); 2] = [(92, 0), (93, 0)];
/// `Wait` (`0x4A1F20`): the first `0x01` the creature whose queue takes it, the second the other,
/// one of the tags `0x58` to `0x5F` its condition (#1279).
const WAIT: u16 = 90;
/// The slots of a scripts' clip that end 0.2 s before the clip does (`0x545204`): the defending
/// row.
const DEFENDING: std::ops::RangeInclusive<u16> = 53..=65;

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
    /// The items of an NPC's `CreateNPC`, their types in its order (its `0x02`s after its type),
    /// and those it holds in its hands: their models, and the items themselves.
    items: Vec<u32>,
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
    /// A fire of the FX objects (`items.pak` category 12), its variant of the smoke class and its
    /// height in the models' units, the record's `0x7E` (`fx`, #1092).
    fx: Option<(u8, f32)>,
    /// An NPC's creature, its `Creature.pak` record's flags and the flags word its record's tags
    /// make (`crowd::flags_word`): how it stands in the others' way.
    creature: (u32, u8, u32),
    /// An NPC's side (`+0x1F0`), by which it is hostile to the hero or not
    /// (`opensacred_rules::minds::hostile_to_heroes`; #580).
    side: u8,
    /// Whether its `CreateNPC` carries a `0x6B` other than 0 ([`RISES`]), which keeps it from
    /// `RegionChange` ([`Placement::changes`]), whether it carries one of 0, which lets the list
    /// of its area turn its type ([`Placement::turns`]), and its record's hostility
    /// (`minds::created_hostility`), by which a turned type takes its side ([`Makers::retype`]).
    rises: bool,
    listed: bool,
    hostility: Option<bool>,
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
    /// Its trades, by its tags ([`TRADES`]; `opensacred_rules::minds::Mind::trader`), and whether
    /// its tags keep it out of its first update's flags ([`KEEPS`]).
    trades: u32,
    kept: bool,
    group: Option<u32>,
    /// Whether the scripts hire it as the hero's follower, or let it go ([`hire_of`]; #657).
    hire: Option<crate::followers::Hire>,
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
    /// An NPC's name in the scripts (`CreateNPC`'s `0x01`, lower-cased), by which their records
    /// name it, and the share of its most health that a `SetHP` gave it, in per cent (#1001).
    name: Option<String>,
    health_share: Option<u32>,
    /// An NPC's dialogue, its name in the dialogues' table, which the hero speaks to it by (`+0x245`,
    /// #1028): its `CreateNPC`'s `0x09`, as its `SetNPCState`s and `SetGroupState`s change it.
    talk: Option<String>,
    /// An NPC's functions of the script manager's list, of which its death runs one (#656):
    /// its `CreateNPC`'s and its `SetNPCState`s' `0x05` and `0x60`.
    on_death: DeathFunctions,
    /// An object's item that lies as a quest's, which the hero picks up, and the function its
    /// taking runs (`CreateObj`'s [`QUEST_ITEM`] and [`ON_TAKE`], #1256).
    quest: Option<Option<String>>,
    /// An object's script name, its `CreateObj`'s first `0x01`, by whose key a quest's item
    /// enters the hero's list of the main quest's items as it is picked up (`sdk_REFKEY`,
    /// `0x44A1C0`; #1257).
    key: Option<String>,
    /// An object of an item's type that lies to be picked up as any lying item, not a quest's
    /// (`CreateObj` without [`QUEST_ITEM`], #1296).
    lies: bool,
}

impl Placement {
    /// Whether a `RegionChange` list may turn an NPC's type as its `CreateNPC` runs: one that
    /// makes no trader and carries a `0x6B` of 0, which a record without the tag does not, its
    /// value then −1 (`0x482F41`, #1063, seen in the original, #1102).
    fn turns(&self) -> bool {
        self.made.1 && !self.object && self.trades == 0 && self.listed
    }

    /// Whether a `RegionChange` may change the creature, a fill's or a `CreateNPC`'s: no trader
    /// (`+0x200` bits `0x4007000`) and none whose `0x6B` above 0 set `+0x2B7` bit `0x08`
    /// (`0x4ABFE0`, #1064).
    fn changes(&self) -> bool {
        !self.object && self.trades == 0 && !self.rises
    }
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

/// `FX_OPAQUE.TGA`, the texture record 203: 16 × 16 texels of nothing, which the items that the
/// original draws by their effects name (the Seraphim's 3809, its glow): such an item is drawn
/// in its model's own material, its effect left out (#754).
const FX_OPAQUE: u32 = 203;

/// The item categories of `items.pak +0x2E` that go in a hand (docs/02 §6.1).
const WEAPON: u8 = 5;
const SHIELD: u8 = 13;
/// The categories of the items that a creature does not wear on its skeleton but holds or carries
/// on a bone of its own: the weapons and the shields, the rings, the amulets and the back's
/// (docs/re/equipment.md, "The bones").
const ATTACHED: [u8; 5] = [WEAPON, SHIELD, 8, 20, 25];
/// The category of the chests, the barrels, the vases, the coffins, the cabinets, the book stands
/// and the shrines, which the client makes `cChest3D`s, whose draw draws their shadows
/// (`0x451300`, `0x5C97B0`), and which take their cells: a creature does not step into a cell
/// that holds one (`0x4F7E10`); the signposts are of 15, `cSpecial3D`, which casts none and
/// takes no cell.
const CHEST: u8 = 4;
/// The category of the doors (`items.pak` 5650 to 5766, `0x428130`).
const DOOR: u8 = 10;
/// The FX objects' category (`cObjectFX`, docs/re/effects.md).
const FX: u8 = 12;
/// A `CreateObj`'s height of an FX object, in the models' units (`0x4861BF`).
const FX_HEIGHT: u8 = 0x7E;
/// `CreateObj`'s mark of a quest's item that it lays in the world, an object of `+0x14` bit
/// `0x800` (`0x485C10`), and the function that its taking runs, kept behind `Take:` (docs/re/
/// netscript.md, "The log's quest items"; #1256).
const QUEST_ITEM: u8 = 0x8E;
const ON_TAKE: u8 = 0x41;
/// A quest's item's quality key as a drop: a plain item's, made by no item maker.
const QUEST_KEY: u8 = 16;
/// The categories of the types that a creature may pick up (`0x427920`; docs/re/items.md,
/// "The walk"): 5 to 9, 13 to 25, 28, 29 and 33.
fn takeable(category: u8) -> bool {
    matches!(category, 5..=9 | 13..=25 | 28 | 29 | 33)
}

/// An item that a `CreateObj` of an item's type without [`QUEST_ITEM`] lays in the world, which the
/// hero picks up as any lying item (seen in the original, #1296): its placement, which is not
/// placed again once taken ([`Npcs::taken`]).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ScriptItem {
    pub index: usize,
}

/// A quest's item that a `CreateObj` of [`QUEST_ITEM`] lays in the world, which the hero picks up
/// as a drop (`pickup`, #1256): the function its taking runs, and its placement, which is not
/// placed again once taken ([`Npcs::taken`]).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct QuestItem {
    pub take: Option<String>,
    pub index: usize,
    /// Its script name (its `CreateObj`'s first `0x01`), by which the test `0x3A` asks for it.
    pub name: Option<String>,
}

/// The name of the game's texts that an item the scripts laid goes by, which the cursor shows in
/// place of its type's (#1360): its `CreateObj`'s first `0x01` of the prefix `res:`, case aside.
/// The scripts' field reader keys such a name alone, any other 0 (`0x4737CA`); the object keeps
/// the key (`+0x3C`), and its name is the key's text where it has one, else its type's
/// (`0x44A250`, from `cEngine::updateWorldCursor`, `0x611E70`).
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct TextName(pub String);

impl TextName {
    /// The text name of the script name `key`; none for a name without the prefix.
    fn of(key: &str) -> Option<Self> {
        match key.get(..4) {
            Some(prefix) if prefix.eq_ignore_ascii_case("res:") => Some(Self(key[4..].to_owned())),
            _ => None,
        }
    }
}
/// The flag of a creature that casts no shadow (`Creature.pak +0x06`, `0x426D30`).
const NO_SHADOW: u8 = 0x10;
/// A `Creature.pak` record's flag of a big type (`0x426CB0`).
const BIG: u8 = 0x02;
/// The weapon kinds of `Weapon.pak +0x1E` held in the left hand: bows and crossbows.
const BOW: u8 = 7;
const CROSSBOW: u8 = 12;
/// The weapon kinds of the guns (`0x54A340`): 13, the Dwarf's cannons, and 14, the muskets.
const GUNS: [u8; 2] = [13, 14];
/// The weapon kinds of the ranged weapons, with which a creature fights at range (`0x427ED0`;
/// docs/re/fight.md, "Who fights"): the bows, the crossbows and the guns.
const RANGED: [u8; 4] = [BOW, CROSSBOW, GUNS[0], GUNS[1]];
/// The arrows' types: the archers' shots' `ARROW_LONG.GRN` (`0x4EB700`) and the Wood Elf's
/// special arrows' (`opensacred_rules::arrows::projectile`).
const ARROWS: [u32; 5] = [arrows::ARROW, 3074, 3075, 3077, 3078];

/// A hand of an NPC's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hand {
    Left,
    Right,
}

/// Where an item's model goes on its holder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mount {
    /// In its hand ([`bones`]).
    Hand,
    /// On its hand's forearm: a shield.
    Forearm,
    /// On its back, by the item's socket `Bone_spine` on its `Bip01 Spine2`, as the choice of the
    /// hero's figures carry their slot 17 (the Seraphim's wings; `0x6F3E90`, #1130).
    Back,
    /// Worn on its skeleton: the item's skinned meshes follow the holder's bones of their joints'
    /// names (`GrannyWearSequence`, docs/re/equipment.md, "Worn items and the classes'
    /// defaults"; [`bind_worn`], #1131).
    Worn,
}

impl Mount {
    /// Where an item of the category `category` goes that its holder holds: a shield on the
    /// forearm, the rest in the hand.
    fn held(category: u8) -> Self {
        if category == SHIELD {
            Self::Forearm
        } else {
            Self::Hand
        }
    }

    /// Whether it goes in or on a hand, which putting the hand's items away takes off.
    fn handed(self) -> bool {
        matches!(self, Self::Hand | Self::Forearm)
    }
}

/// An item an NPC or the hero holds.
#[derive(Clone, Debug)]
pub struct Held {
    /// The model, relative to the asset directory.
    pub(crate) model: String,
    /// The texture its item names (`+0x08`).
    pub(crate) texture: Option<String>,
    hand: Hand,
    mount: Mount,
    /// The slot of its wearer's equipment that it is worn in, for an item not in a hand: which
    /// an item put on in that slot takes off, as it does a class's default (`0x556A30`, #1307).
    slot: Option<u8>,
}

impl Held {
    /// The item of the model `model` and the texture `texture` worn in the slot `slot` of the
    /// hero's equipment (#1307): on its back in the slot 17, the wings', else on its skeleton
    /// (docs/re/equipment.md, "The slots").
    pub fn worn(slot: u8, model: String, texture: Option<String>) -> Self {
        Self {
            model,
            texture,
            hand: Hand::Right,
            mount: if slot == BACK {
                Mount::Back
            } else {
                Mount::Worn
            },
            slot: Some(slot),
        }
    }

    /// The slot of its wearer's equipment that it is worn in, none in a hand.
    pub fn slot(&self) -> Option<u8> {
        self.slot
    }
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

/// The skill whose level lessens the shortfall of a creature's weapon's level, by the class of
/// what it holds `class` and its items in their hands with their values, left hand first
/// (`opensacred_rules::fight::weapon_skill`, `0x578ED0`; #844).
fn skill_of(class: u8, holding: &[(InHand, Option<fight::ItemValues>)]) -> u8 {
    let family = |held: &(InHand, Option<fight::ItemValues>)| held.1.map_or(0, |v| v.family);
    let right = holding
        .iter()
        .find(|held| held.0.hand == Hand::Right)
        .map(family);
    let mut weapons: Vec<&(InHand, Option<fight::ItemValues>)> = holding
        .iter()
        .filter(|held| held.0.category == WEAPON)
        .collect();
    weapons.sort_by_key(|held| held.0.hand != Hand::Left);
    let weapons: Vec<(u8, u8)> = weapons
        .into_iter()
        .map(|held| (family(held), held.0.kind.unwrap_or(0)))
        .collect();
    opensacred_rules::fight::weapon_skill(class, right, &weapons)
}

/// Whether an NPC holds a ranged weapon, its items in their hands ([`ranged`]).
fn ranged_of(holding: &[InHand]) -> bool {
    holds_kind(holding, &RANGED)
}

/// Whether a creature holds a weapon of one of `kinds`, its items in their hands.
fn holds_kind(holding: &[InHand], kinds: &[u8]) -> bool {
    holding
        .iter()
        .any(|held| held.category == WEAPON && held.kind.is_some_and(|kind| kinds.contains(&kind)))
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
        trades: 0,
        kept: false,
        group: None,
        hire: None,
        heading: 0.0,
        object: false,
        casts: kind.casts,
        blocks: false,
        door: false,
        fx: None,
        trigger: None,
        creature: (creature, kind.flags, 0),
        side: kind.class,
        rises: false,
        listed: false,
        hostility: None,
        texture: kind.texture.clone(),
        layer: 0,
        items: Vec::new(),
        equipment: Vec::new(),
        holding: Vec::new(),
        rank: fight::Rank::Common,
        tag: 0,
        clips: ClipNames::of(slots, held_class(&[])),
        plain: false,
        name: None,
        health_share: None,
        talk: None,
        on_death: DeathFunctions::default(),
        quest: None,
        key: None,
        lies: false,
    }
}

#[derive(Resource)]
pub struct Npcs {
    placements: Vec<Placement>,
    by_sector: HashMap<(u32, u32), Vec<usize>>,
    /// The cells of the triggers ([`trigger_cells`]), by sector, and by trigger; the triggers
    /// open, those made open and those that a door or the scripts have opened since (#372,
    /// #952), whose cells a sector that comes in leaves free, the others' closed; and those that
    /// a hero has turned, whose doors no townsperson closes.
    closed: HashMap<(u32, u32), Vec<(String, Spot)>>,
    triggers: HashMap<String, Vec<Spot>>,
    opened: HashSet<String>,
    marked: HashSet<String>,
    /// The triggers locked (`0x8000` of their state, #1006), whose doors open to nobody.
    locked: HashSet<String>,
    /// The trigger of each object that the scripts name (`CreateObj`'s `0x01` and `0x29`), by
    /// the object's name lower-cased, which `SetObjState` sets (#952).
    named: HashMap<String, String>,
    /// What the scripts' records are placed with, the places that their `DefPos`s name, and the
    /// placements that the play's records made whose sectors were in the world (#1014).
    makers: Makers,
    places: HashMap<String, (i32, i32)>,
    pending: Vec<usize>,
    /// The placements that the play's records made, which live wherever they stand from then on
    /// ([`Self::living_groups`]).
    played: HashSet<usize>,
    /// The placements of the quests' items that the hero took, which their sectors do not place
    /// again ([`QuestItem`], #1256).
    taken: HashSet<usize>,
    /// The dialogues' table by name, as the start's records fill it, its first entry twice
    /// (`0x476A05`), which the play's `SetGroupState`s look their dialogues up in (#1021), and
    /// each entry's icon, as the scripts' `SetIcon`s leave it (#1039).
    dialogues: Vec<String>,
    icons: Vec<i32>,
    /// The dialogue of each creature type that the start's `SetRgnDialog`s give, all of the
    /// region 0 (`0x465280`, #1028).
    region_talks: HashMap<u32, String>,
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
    /// The arrows' models, textures and sizes by their types ([`ARROWS`]).
    arrows: HashMap<u32, (String, Option<String>, f32)>,
    /// The fractions of the attack's clips at which they hit, by the clips' names upper-cased
    /// (`MotionDesc +0xA0`, `+0xA4`: `floats_62[1]` and `[2]` of `tables/motions.ron`).
    hits: HashMap<String, [f32; 2]>,
    spawned: HashSet<(u32, u32)>,
    /// What became of the creatures that went with their sectors ([`Kept`]).
    kept: Kept,
    /// The sectors come in with no creatures kept whose waters' crabs and fish wait for the
    /// sectors about them ([`swimmers`]).
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
    /// at its next coming in while the region that filled last is its own (`0x4954C0`,
    /// `0x4939F0`, [`Kept::enter`]).
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
    /// The sectors that kept a time, keeping a fill's living creature as they went (`+0x59C`).
    times: HashSet<(u32, u32)>,
    /// The region whose sector filled last (`0x94E080`).
    last: Option<u16>,
}

/// What a sector that comes in does with its fills ([`Kept::enter`]).
struct Entered {
    /// The fills' living creatures that went with it, put back where they stood.
    back: Vec<(Placement, Stood)>,
    /// Whether its fills make their creatures anew.
    fill: bool,
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

    /// What sector `at` does with its fills as it comes in, `filling` its region where it has
    /// fills, `region_of` another sector's (`FillSector`, `0x47CCEC`…`0x47CDEF`, docs/re/
    /// spawning.md): where it kept a time and its region filled last, the creatures it kept come
    /// back, none where it kept none, and it fills nothing; else it fills anew, dropping what it
    /// kept, and a region other than the last drops what every sector of another region kept,
    /// and its time. A sector of no fills puts back what it kept (`0x4939F0`).
    fn enter(
        &mut self,
        at: (u32, u32),
        filling: Option<u16>,
        region_of: impl Fn((u32, u32)) -> u16,
    ) -> Entered {
        let back = self.filled.remove(&at).unwrap_or_default();
        let Some(region) = filling else {
            return Entered { back, fill: false };
        };
        if self.times.contains(&at) && self.last == Some(region) {
            return Entered { back, fill: false };
        }
        if self.last != Some(region) {
            bevy::log::debug!("npcs: the region {region} fills after {:?}", self.last);
            self.filled.retain(|&sector, _| region_of(sector) == region);
            self.times.retain(|&sector| region_of(sector) == region);
            self.last = Some(region);
        }
        Entered {
            back: Vec::new(),
            fill: true,
        }
    }

    /// Notes that a placed creature is spawned.
    fn spawned(&mut self, placed: &Placed) {
        if let Source::Kept(index) = placed.source {
            self.out.insert(index);
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
                // Kept with its sector, which keeps a time then (`0x4954C0`).
                if let Some(stood) = stood {
                    self.filled
                        .entry(placed.at)
                        .or_default()
                        .push(((**placement).clone(), stood));
                    self.times.insert(placed.at);
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
    /// Its model's file, by which its clips of any slot are found (`motions`).
    model: String,
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

impl Npc {
    /// The factor of its walk and run speeds and of their clips' rate ([`repace`]).
    pub fn pace(&self) -> f32 {
        self.pace
    }

    /// Whether its dying clip `choice`, as [`die`] plays it, has played to its end: at once for
    /// a model of none, never before its clips are made.
    pub(crate) fn died(&self, choice: u32, players: &Query<&AnimationPlayer>) -> bool {
        let (Some(graph), Some(player)) = (&self.graph, self.player) else {
            return false;
        };
        let Some(&node) = graph
            .dying
            .get(usize::try_from(choice).unwrap_or(0) % graph.dying.len().max(1))
        else {
            return true;
        };
        players.get(player).is_ok_and(|player| {
            player
                .animation(node)
                .is_some_and(|clip| clip.is_finished())
        })
    }

    /// The clip that its walk `walk` plays now, its animation player, its node and its name, and
    /// whether it runs (`steps`).
    pub(crate) fn walking(&self, walk: &Walk) -> Option<(Entity, AnimationNodeIndex, &str, bool)> {
        let graph = self.graph.as_ref()?;
        let node = graph.moving(walk)?;
        let runs = walk.runs && graph.run.is_some();
        let name = if runs {
            self.names.run.as_deref()
        } else {
            self.names.walk.as_deref()
        }?;
        Some((self.player?, node, name, runs))
    }
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
    /// The clips of the slots that the scripts' `PlayAnim`s played, added to its graph as they
    /// come (#1277), each with its clip.
    scripted: HashMap<u16, (AnimationNodeIndex, Handle<AnimationClip>)>,
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
    /// Whether it walks with the AI's state 0 (`0x542B20`) rather than under an order of its own
    /// (a behaviour, a fight, a door, a master): a script's `NPC_Goto` (seen as the action 1 with
    /// the AI's state 0, #977), the idle action's step, a parting or a step aside. One that walks
    /// so does not step aside for the hero (`0x547070`, `ai::aside`).
    idle: bool,
    /// The leg it heads for and its course there, found as it set out on the leg
    /// (`paths::Course`, `0x4F7710`), so that a creature that steps into its way afterwards
    /// stops it rather than turns it round (#989).
    course: Option<(Vec3, paths::Course<(Level, bool)>)>,
    /// The level it walks onto, where it is given (`0x4F7710`'s position's floor): a follower's
    /// step off's (#1013); else any its way reaches the target's cell on.
    onto: Option<Level>,
}

/// How long a creature whose walk could not start stands before its walk ends, in seconds.
const BALKS: f32 = 1.0;

/// How near the end of a leg short of its target a walker sets out on the next leg, in world
/// units: its last step lands on the leg's point.
const AT_LEG: f32 = 1.0;

/// A creature that stays where it is made, the waters' crab and fish (`minds::still`): a walk
/// that anything orders it ends as it starts (`0x4F4770` at `0x4F47DA`, #967).
#[derive(Component)]
pub struct Still;

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
            idle: false,
            course: None,
            onto: None,
        }
    }

    /// The walk onto the level `level`, its way to the target's cell on it.
    pub fn onto(self, level: Level) -> Self {
        Self {
            onto: Some(level),
            ..self
        }
    }

    /// The walk, one with the AI's state 0, which does not step aside for the hero.
    pub fn idle(self) -> Self {
        Self { idle: true, ..self }
    }

    /// Whether it walks with the AI's state 0.
    pub fn is_idle(&self) -> bool {
        self.idle
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

/// The hero's start: the global cell of the `StartPosition` record that `tree`'s start script
/// ran (`StartPosition 0B:x 0B:y`), if it ran one.
pub fn start_position(tree: &Tree) -> Option<((u32, u32), Option<u8>)> {
    tree.start
        .iter()
        .filter(|r| r.kind == START_POSITION)
        .find_map(|r| {
            let Body::Operands(ops) = &r.body else {
                return None;
            };
            let x = u32::try_from(value_i32(&ops.first()?.value)?).ok()?;
            let y = u32::try_from(value_i32(&ops.get(1)?.value)?).ok()?;
            // Its floor, where it names one: the Vampiress's 2 (#1017).
            let floor = ops
                .get(2)
                .and_then(|op| value_i32(&op.value))
                .and_then(|floor| u8::try_from(floor).ok());
            Some(((x, y), floor))
        })
}

/// The records that place the world's NPCs and objects: the start script's as it ran, with
/// those of the sections of the quests that its `SetUpQuest`s and `TriggerQuest`s ran in their
/// middle, the givers of `OnSetUp` and the entered quests' `OnEnter` among them (`scripts`,
/// `0x48F7F0`, `0x46C160`), then the sectors' `Init` and `Enter` functions' and the regions'
/// `Init` functions', which the engine runs as their sectors come near (docs/re/spawning.md,
/// `0x492880`); each record's NPC or object is placed as the sector of its cell comes in.
fn start_records(tree: &Tree) -> Vec<&Record> {
    let mut records: Vec<&Record> = tree.start.iter().collect();
    for sector in tree.functions.iter().filter(|f| of_sectors(&f.name)) {
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
    /// The sound profile of its own type (`items.pak +0x24`), not its parent's: 0 for a fixed
    /// item without a slot of its own, as the heroes' starting weapons, whose hits the original
    /// sounds by their swing alone (#709, #712).
    sound: u32,
    /// Its size (`items.pak +0x14`, 50 for 0, `arrows::size`): a held item's for the hurled
    /// blades' touch (`0x600340`, `0x428CE0`; `making::Blade`).
    size: f32,
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
        .filter(|r| r.kind == WEAPON_POOL)
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
/// items, the first swapped ([`SWAPPED`]), and a crossbow or a musket first that its model of the
/// shots `shots` cannot shoot replaced by `weapon` of the kind it asks for, none if it gives none
/// (`spawning::filled_weapon`, `0x480547`, #1217); the second only after a weapon of a kind that
/// leaves a hand free; the third, a worn piece, is left out, the game drawing no worn models.
fn gear_of<'a>(
    rules: &[Rule],
    creature: u32,
    (pools, gear): (&HashMap<i32, Vec<u32>>, &'a HashMap<u32, Gear>),
    (shots, weapon): (
        spawning::Shots,
        impl FnOnce(u8, &mut spawning::Rng) -> Option<u32>,
    ),
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
    let first = first.and_then(|item| {
        match spawning::filled_weapon(gear.get(&item).and_then(|g| g.kind), shots) {
            Some(kind) => weapon(kind, rng),
            None => Some(item),
        }
    });
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
pub fn hit_fractions(tables: &Tables) -> HashMap<String, [f32; 2]> {
    tables
        .motions()
        .unwrap_or_default()
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
pub fn hit_points(tables: &Tables) -> HashMap<String, Vec<f32>> {
    tables
        .motions()
        .unwrap_or_default()
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

/// Whether the function `name` runs as a sector comes near: a sector's `Init` or `Enter`
/// ([`sector_function`]), or a region's `Init`, `Region<n>Init`, which runs before the first
/// `Init` of a sector of its region (`0x492880`, `scripts`).
fn of_sectors(name: &str) -> bool {
    sector_function(name).is_some()
        || name
            .strip_prefix("Region")
            .and_then(|rest| rest.strip_suffix("Init"))
            .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

/// The sector of a function of the sectors', by its name, `Sector<x><yyy>Init` or `…Enter` as the
/// engine makes it (`Sector%d%3.3dInit`, `0x94F088`); `None` for any other function, and for the
/// few named another way (`Sector28Enter`), which the engine never asks for.
pub(crate) fn sector_function(name: &str) -> Option<(u32, u32)> {
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
/// The group of a `CreateNPC` of the operands `ops` (#669): its last tag [`GROUP`]'s number, if
/// it is not 0, kept apart from the fills' packs, whose numbers stay below [`SCRIPT_GROUP`].
fn script_group(ops: &[opensacred_data::netscript::Operand]) -> Option<u32> {
    ops.iter()
        .rev()
        .find_map(|o| match (o.tag, &o.value) {
            (GROUP, Value::I32(v)) => Some(u32::try_from(*v).unwrap_or(0)),
            _ => None,
        })
        .filter(|&n| n != 0)
        .map(|n| SCRIPT_GROUP | (n & !SCRIPT_GROUP))
}

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
    /// The trades that its tags `0x23`, `0x24`, `0x25` and `0x64` give it ([`TRADES`]), and
    /// whether `0x64` stamps its trader's dialogue.
    trades: u32,
    stamped: bool,
    /// Its home (`0x4D`, `+0x2B9`): the cell whose centre it goes home to.
    home: Option<(u32, u32)>,
    /// The dialogue it gives the creature, by its name, or none where it takes its dialogue away
    /// ([`Self::dialogue`]; #1028).
    talk: Option<Option<String>>,
    /// Its death's and sight functions, in their order: each `0x05`'s (`true`), which replaces
    /// those the creature has, and `0x60`'s, which adds a sight function (`0x461FBC`, `0x46204B`;
    /// #656, #1025).
    deaths: Vec<(bool, String)>,
    /// Its heading in degrees (`0x03`), turned as `CreateNPC` turns it (`0x44DDC0`); its group
    /// (`0x11`, `+0x249`, `0x4A9EF0`; [`script_group`]); and whether it joins a trader's stock
    /// of horses (`0x63`, the list at `+0x79E8`), which takes its dialogue away (#658).
    heading: Option<f32>,
    group: Option<u32>,
    stocked: bool,
    /// Whether it hires its creature as the hero's follower, or lets it go ([`hire_of`]; #657).
    hire: Option<crate::followers::Hire>,
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
            group: script_group(ops),
            hire: hire_of(ops),
            ..Self::default()
        };
        for op in ops {
            match (op.tag, &op.value) {
                (DIALOGUE, Value::Str(name)) => {
                    let index = dialogues.iter().position(|d| d.eq_ignore_ascii_case(name));
                    let found = index.filter(|&i| i > 0).and_then(|i| dialogues.get(i));
                    state.dialogue = Some(found.is_some());
                    state.talk = Some(found.cloned());
                }
                (DIALOGUE_NUMBER, _) => {
                    state.dialogue = Some(false);
                    state.talk = Some(None);
                }
                (0x08, Value::Marker) => state.hostile = Some(true),
                (0x0E, Value::Marker) => state.hostile = Some(false),
                (tag, Value::Marker) if trade(tag) != 0 => {
                    state.trades |= trade(tag);
                    state.stamped |= tag == STAMPED;
                }
                (HOME, Value::I32x3([x, y, _])) => {
                    if let (Ok(x), Ok(y)) = (u32::try_from(*x), u32::try_from(*y)) {
                        state.home = Some((x, y));
                    }
                }
                (ON_DEATH, Value::Str(name)) => state.deaths.push((true, name.clone())),
                (ON_SIGHT, Value::Str(name)) => state.deaths.push((false, name.clone())),
                (0x03, Value::U16(degrees)) => state.heading = Some(f32::from(*degrees)),
                (STOCK, Value::Str(_)) => state.stocked = true,
                _ => {}
            }
        }
        state
    }

    /// Whether it gives a creature made before what the game follows on one (#656, #658): its
    /// death's functions, its heading, its group or a trader's stock.
    fn restates(&self) -> bool {
        !self.deaths.is_empty()
            || self.heading.is_some()
            || self.group.is_some()
            || self.stocked
            || self.talk.is_some()
            || self.hire.is_some()
    }

    /// The dialogue it leaves a creature that had `talk`: its own, or none for a horse it stocks.
    fn talk_of(&self, talk: Option<String>) -> Option<String> {
        let talk = self.talk.clone().unwrap_or(talk);
        talk.filter(|_| !self.stocked)
    }

    /// Gives `placement` its death's functions, its heading, its group and, joining a stock, no
    /// dialogue ([`Self::restates`]).
    fn restate_placement(&self, placement: &mut Placement) {
        self.restate(&mut placement.on_death);
        if let Some(heading) = self.heading {
            placement.heading = heading;
        }
        if self.group.is_some() {
            placement.group = self.group;
        }
        if self.stocked {
            placement.dialogue = false;
        }
        placement.talk = self.talk_of(placement.talk.take());
        if let Some(talk) = &self.talk {
            placement.dialogue = talk.is_some() && !self.stocked;
        }
        if self.hire.is_some() {
            placement.hire = self.hire;
        }
    }

    /// Gives `functions` its death's functions ([`Self::deaths`]).
    fn restate(&self, functions: &mut DeathFunctions) {
        for (replaces, name) in &self.deaths {
            if *replaces {
                functions.set(name);
            } else {
                functions.see(name);
            }
        }
    }

    /// Applies it to `placement`, in the order of `0x461540`: the flags word, which takes a
    /// trader's trade away with the mode `0x800`; the home; the side by the hostility; the
    /// dialogue; the level, 1000 a boss; the trades, `0x64`'s dialogue last.
    fn apply(&self, placement: &mut Placement) {
        let (creature, _, word) = &mut placement.creature;
        let mut stated = self.word;
        if stated & 0x800 != 0 && placement.trades != 0 {
            stated &= !0x800;
            placement.trades = 0;
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
        placement.trades |= self.trades;
        placement.dialogue |= self.stamped;
        self.restate_placement(placement);
    }
}

/// The bits of `+0x200` of the trade that the tag `tag` gives ([`TRADES`]), 0 for none.
fn trade(tag: u8) -> u32 {
    TRADES
        .iter()
        .find(|(traded, _)| *traded == tag)
        .map_or(0, |&(_, bit)| bit)
}

/// Whether the operands `ops` carry a `0x6B` other than 0 ([`RISES`]).
fn rises(ops: &[opensacred_data::netscript::Operand]) -> bool {
    ops.iter().any(|op| {
        op.tag == RISES
            && !matches!(
                op.value,
                Value::I32(0) | Value::U16(0) | Value::U8(0) | Value::Marker
            )
    })
}

/// Whether the operands `ops` carry a `0x6B` of 0 ([`RISES`]).
fn listed(ops: &[opensacred_data::netscript::Operand]) -> bool {
    ops.iter().any(|op| {
        op.tag == RISES
            && matches!(
                op.value,
                Value::I32(0) | Value::U16(0) | Value::U8(0) | Value::Marker
            )
    })
}

/// The pair of the area `area`'s `RegionChange` list that a play's `CreateNPC` of the operands
/// `ops` took as it ran, as `scripts` marks it ([`AREA_TURN`]; #1063), none where it took none.
fn area_turn(ops: &[opensacred_data::netscript::Operand], area: u8) -> Option<quests::Turn> {
    let from = ops.iter().find_map(|op| match (op.tag, &op.value) {
        (0x02, Value::I32(creature)) => u32::try_from(*creature).ok(),
        _ => None,
    })?;
    ops.iter().find_map(|op| match (op.tag, &op.value) {
        (AREA_TURN, Value::I32x3([turned, to, flags])) if *turned == i32::from(area) => {
            Some(quests::Turn {
                from,
                to: u32::try_from(*to).ok()?,
                flags: u16::try_from(*flags).ok()?,
            })
        }
        _ => None,
    })
}

/// What the tags of a `CreateNPC` or a `SetNPCState` of operands `ops` do about the hero's
/// followers (#657, [`HIRES`]): the bit `0x100` hires, keeping it fighting with `0x200` (`0x4B`)
/// and else of the flag 4 (`0x37`, `0x71`; `0x462A99`); without it, `0x400` lets it go
/// (`0x462BB9`).
fn hire_of(ops: &[opensacred_data::netscript::Operand]) -> Option<crate::followers::Hire> {
    use crate::followers::Hire;
    let bits = ops.iter().fold(0, |bits, op| {
        match HIRES.iter().find(|(tag, ..)| *tag == op.tag) {
            Some(&(_, sets, clears)) => bits & !clears | sets,
            None => bits,
        }
    });
    if bits & 0x100 != 0 {
        Some(if bits & 0x200 != 0 {
            Hire::Fights
        } else {
            Hire::Escorts
        })
    } else if bits & 0x400 != 0 {
        Some(Hire::LetsGo)
    } else {
        None
    }
}

/// The dialogues' table of `records` by name, as their entries of type 40 fill it, the first
/// twice (`0x476A05`), and each entry's icon, its body's second value (`+0x48`, #1039).
fn dialogue_table(records: &[&Record]) -> (Vec<String>, Vec<i32>) {
    let mut table = (Vec::new(), Vec::new());
    for record in records {
        if let Body::Dialog { name, values } = &record.body {
            if table.0.is_empty() {
                table.0.push(name.clone());
                table.1.push(values[1]);
            }
            table.0.push(name.clone());
            table.1.push(values[1]);
        }
    }
    table
}

/// Gives the first entry of `dialogues` of each name of `changes`, case aside, its icon in
/// `icons`, in order (`0x4A1A50`); a name the table does not hold changes none.
fn reicon(dialogues: &[String], icons: &mut [i32], changes: Vec<(String, i32)>) {
    for (name, icon) in changes {
        match dialogues.iter().position(|d| d.eq_ignore_ascii_case(&name)) {
            Some(index) => {
                if let Some(entry) = icons.get_mut(index) {
                    *entry = icon;
                }
            }
            None => bevy::log::debug!("npcs: SetIcon of a dialogue {name} not in the table"),
        }
    }
}

/// Each `SetIcon` of `records` (86, `0x4A1A50`; #1039), in order: its dialogue's name (`0x01`)
/// and its icon (`0x0B`). One without a name changes the running dialogue's, which `scripts`
/// names for it.
fn set_icons<'a>(records: impl IntoIterator<Item = &'a Record>) -> Vec<(String, i32)> {
    let mut icons = Vec::new();
    for record in records {
        let Body::Operands(ops) = &record.body else {
            continue;
        };
        if record.kind != SET_ICON {
            continue;
        }
        let name = ops.iter().find_map(|o| match (o.tag, &o.value) {
            (0x01, Value::Str(name)) => Some(name.clone()),
            _ => None,
        });
        let icon = ops.iter().find_map(|o| match (o.tag, &o.value) {
            (0x0B, Value::I32(icon)) => Some(*icon),
            _ => None,
        });
        if let (Some(name), Some(icon)) = (name, icon) {
            icons.push((name, icon));
        }
    }
    icons
}

/// The dialogue of each creature type that `records`' `SetRgnDialog`s give (106, `0x465280`): its
/// `0x02`'s type and its `0x09`'s name, a later one of the type replacing an earlier.
fn region_talks(records: &[&Record]) -> HashMap<u32, String> {
    let mut talks = HashMap::new();
    for record in records.iter().filter(|r| r.kind == SET_RGN_DIALOG) {
        let Body::Operands(ops) = &record.body else {
            continue;
        };
        let creature = ops.iter().find_map(|o| match (o.tag, &o.value) {
            (0x02, Value::I32(t)) => u32::try_from(*t).ok(),
            _ => None,
        });
        let name = ops.iter().find_map(|o| match (o.tag, &o.value) {
            (DIALOGUE, Value::Str(name)) => Some(name.clone()),
            _ => None,
        });
        if let (Some(creature), Some(name)) = (creature, name) {
            talks.insert(creature, name);
        }
    }
    talks
}

/// A `SetGroupState`'s change of its group's creatures (73, `0x49E760`; #1021,
/// docs/re/netscript.md, "`SetGroupState`"): its group, whether it removes its living members
/// (`0x51`), and else the flags word of its operand-less tags ([`ways::state_word`]), its
/// hostility, whether it gives a dialogue (an index above 0 by its `0x09`'s name; none, or a
/// `0x0A`, takes the members' away), its death's and sight functions and its level.
#[derive(Clone, Debug, Default, PartialEq)]
struct GroupState {
    group: Option<u32>,
    removes: bool,
    word: u32,
    hostile: Option<bool>,
    dialogue: bool,
    /// The dialogue it gives, by its name (#1028).
    talk: Option<String>,
    death: Option<String>,
    sight: Option<String>,
    level: u32,
}

impl GroupState {
    /// The change of a `SetGroupState` of operands `ops`, its dialogue looked up in the
    /// dialogues' table `dialogues`.
    fn of(ops: &[opensacred_data::netscript::Operand], dialogues: &[String]) -> Self {
        let markers = ops
            .iter()
            .filter(|o| matches!(o.value, Value::Marker))
            .map(|o| o.tag);
        let mut state = Self {
            group: script_group(ops),
            word: ways::state_word(markers),
            level: level_tag(ops),
            ..Self::default()
        };
        for op in ops {
            match (op.tag, &op.value) {
                (REMOVES, Value::Marker) => state.removes = true,
                (0x08, Value::Marker) => state.hostile = Some(true),
                (0x0E, Value::Marker) => state.hostile = Some(false),
                (ON_DEATH, Value::Str(name)) => state.death = Some(name.clone()),
                (ON_SIGHT, Value::Str(name)) => state.sight = Some(name.clone()),
                (DIALOGUE, Value::Str(name)) => {
                    state.talk = dialogues
                        .iter()
                        .position(|entry| entry.eq_ignore_ascii_case(name))
                        .filter(|&index| index > 0)
                        .and_then(|index| dialogues.get(index))
                        .cloned();
                    state.dialogue = state.talk.is_some();
                }
                (DIALOGUE_NUMBER, _) => {
                    state.dialogue = false;
                    state.talk = None;
                }
                _ => {}
            }
        }
        state
    }

    /// Gives a member's placement the change, in the order of `0x49E760`: the flags word, the
    /// side, the sight function, the dialogue, the level, the death's function.
    fn apply(&self, placement: &mut Placement) {
        let (creature, _, word) = &mut placement.creature;
        *word = ways::grouped_word(*word, self.word);
        if let Some(hostile) = self.hostile {
            placement.side = minds::grouped_side(placement.side, *creature, self.word, hostile);
        }
        if let Some(sight) = &self.sight {
            placement.on_death.see(sight);
        }
        placement.dialogue = self.dialogue;
        placement.talk.clone_from(&self.talk);
        if self.level > 0 {
            placement.tag = self.level;
            if self.level == fight::BOSS_TAG {
                placement.rank = fight::Rank::Boss;
            }
        }
        if let Some(death) = &self.death {
            placement.on_death.set(death);
        }
    }

    /// Gives a member in the world the change: its mind's flags word, side and dialogue, how it
    /// stands in the hero's way, and its functions; its level is not changed in the world.
    fn apply_to(
        &self,
        mind: &mut ai::Mind,
        way: Option<&mut crowd::InTheWay>,
        functions: &mut DeathFunctions,
    ) {
        mind.flags = ways::grouped_word(mind.flags, self.word);
        if let Some(hostile) = self.hostile {
            mind.side = minds::grouped_side(mind.side, mind.creature, self.word, hostile);
        }
        mind.dialogue = self.dialogue;
        if let Some(way) = way {
            way.stands.hostile = ways::stops_the_hero(mind.flags);
        }
        if let Some(sight) = &self.sight {
            functions.see(sight);
        }
        if let Some(death) = &self.death {
            functions.set(death);
        }
    }
}

/// Puts the play's `CreateNPC`s and `CreateObj`s placed at a creature's place (`0x04` of
/// `CPOS:HERO` or `CPOS:<name>`, the field reader at `0x474222`, `0x474295`; #1046) on that
/// creature's cell and floor as the records run, as `at` finds them by the creature's name, the
/// hero's for none; one whose creature is not found keeps its name, which places nothing. The
/// original moves a place at the hero on the ground to a free point about it where another object
/// stands on it or no way leads there (`0x45FF80`), which is not followed.
fn locate(records: &mut [Record], at: impl Fn(Option<&str>) -> Option<(i32, i32, i32)>) {
    for record in records
        .iter_mut()
        .filter(|record| record.kind == CREATE_NPC || record.kind == CREATE_OBJ)
    {
        let Body::Operands(ops) = &mut record.body else {
            continue;
        };
        for op in ops.iter_mut().filter(|op| op.tag == 0x04) {
            let (Value::Name(name) | Value::Str(name)) = &op.value else {
                continue;
            };
            let lower = name.to_ascii_lowercase();
            let Some(creature) = lower.strip_prefix(CREATURE_POSITION) else {
                continue;
            };
            let found = at((creature != "hero").then_some(creature));
            if let Some((x, y, floor)) = found {
                op.value = Value::I32x3([x, y, floor]);
            }
        }
    }
}

/// Where the play's `NPC_Goto` or `Teleport` sends a creature (#1035): a global cell and its floor,
/// or the place of a creature, `None` the hero's (`CPOS:<name>`).
#[derive(Clone, Debug, PartialEq)]
enum Whither {
    Cell((i32, i32, i32)),
    Creature(Option<String>),
}

/// A creature that the play's `NPC_Goto` sends walking (`walks`) or its `Teleport` puts somewhere
/// ([`Npcs::moves`]), at once or in its turn of its queue under the anim mode (`queued`).
#[derive(Clone, Debug, PartialEq)]
struct Moved {
    name: String,
    act: Act,
    queued: bool,
}

/// What a record of the play does to a creature (#1220, #1277).
#[derive(Clone, Debug, PartialEq)]
enum Act {
    /// `NPC_Goto` (a walk) or `Teleport`, to `whither`.
    Go { whither: Whither, walks: bool },
    /// `PlayAnim`: the slot of its clip, and how many times it plays outside the anim mode.
    Clip { slot: u16, loops: u32 },
    /// A `CallFunktion` naming the creature, which the scripts deferred under the anim mode: its
    /// function runs at its turn (#1278).
    Call(String),
    /// `Wait` under the anim mode: the other creature, `None` the hero, and its condition
    /// (#1279).
    Wait(Option<String>, Until),
    /// Another record that waits its turn under the anim mode, of the kind: its turn taken, the
    /// record itself carried out elsewhere or not yet (#1278 to #1281).
    Other(u16),
    /// `SetCinemaMode`'s stop of the hero where its queue is empty (`0x47740E`, `0x54E6D0(1, 0)`;
    /// #1280).
    Halt,
}

/// The creature that a play's `NPC_Goto` or `Teleport` of the operands `ops` moves, and whether
/// it walks there, under the anim mode or not (`queued`): `NPC_Goto` the creature of its last
/// `0x01` (`0x49E210` keeps each over the one before; the Daemon's start names `HERO` and then
/// Anducar, #1280) and none without one, `Teleport` the creature of its first and the hero for
/// none (`0x491D40`); the hero's teleport outside the mode left to `scripts`, which puts it at
/// once (#1035).
fn mover(kind: u16, ops: &[opensacred_data::netscript::Operand], queued: bool) -> Option<String> {
    match kind {
        NPC_GOTO => ops.iter().rev().find_map(|o| match (o.tag, &o.value) {
            (0x01, Value::Str(name)) => Some(name.to_ascii_lowercase()),
            _ => None,
        }),
        TELEPORT => Some(name_of(ops).unwrap_or_else(|| HERO.to_owned()))
            .filter(|name| queued || name != HERO),
        _ => None,
    }
}

/// The hero's name in the records (`HERO`, case aside), lower case.
const HERO: &str = "hero";

/// A `PlayAnim` of the operands `ops` (`0x4A2550`): the creature of its first `0x01`, the slot of
/// its first `0x0B` and the count of its second, 1 by default; none for the hero, for no creature
/// or slot above 0, or for the name `schmiede`, which opens the smith's window instead. Under the
/// anim mode (`queued`), another record that waits its turn there and names a creature
/// ([`QUEUED_OTHERS`]).
fn played(
    kind: u16,
    ops: &[opensacred_data::netscript::Operand],
    queued: bool,
    now: f64,
) -> Option<Moved> {
    let names: Vec<String> = ops
        .iter()
        .filter_map(|o| match (o.tag, &o.value) {
            (0x01, Value::Str(name)) => Some(name.to_ascii_lowercase()),
            _ => None,
        })
        .collect();
    if kind == PLAY_ANIM {
        let numbers: Vec<i32> = ops
            .iter()
            .filter_map(|o| match (o.tag, &o.value) {
                (0x0B, Value::I32(n)) => Some(*n),
                _ => None,
            })
            .collect();
        let name = names
            .first()
            .filter(|name| *name != "hero" && *name != "schmiede")?;
        let slot = u16::try_from(*numbers.first()?).ok().filter(|&s| s > 0)?;
        let loops = numbers
            .get(1)
            .and_then(|&n| u32::try_from(n).ok())
            .unwrap_or(1)
            .max(1);
        return Some(Moved {
            name: name.clone(),
            act: Act::Clip { slot, loops },
            queued,
        });
    }
    // A deferred `CallFunktion`: the function first, the creature second (`scripts`).
    if kind == crate::scripts::CALL_FUNCTION {
        let (function, name) = (names.first()?, names.get(1)?);
        return Some(Moved {
            name: name.clone(),
            act: Act::Call(function.clone()),
            queued: true,
        });
    }
    // A `Wait`, which outside the mode does nothing.
    if kind == WAIT {
        let name = names.first().filter(|name| *name != "hero")?;
        let other = names.get(1).filter(|other| *other != "hero").cloned();
        let until = ops.iter().find_map(|o| {
            let n = match o.value {
                Value::I32(n) => n,
                _ => 0,
            };
            Until::of(o.tag, n, now)
        })?;
        return queued.then(|| Moved {
            name: name.clone(),
            act: Act::Wait(other, until),
            queued,
        });
    }
    let &(_, rank) = QUEUED_OTHERS.iter().find(|(other, _)| *other == kind)?;
    let name = names.get(rank).filter(|name| *name != "hero")?;
    queued.then(|| Moved {
        name: name.clone(),
        act: Act::Other(kind),
        queued,
    })
}

/// Where a record's position of the tag `tag` sends a creature: a cell of its own, a `DefPos`'s
/// name, or a creature's place.
fn whither(
    ops: &[opensacred_data::netscript::Operand],
    tag: u8,
    named: &HashMap<String, (i32, i32)>,
) -> Option<Whither> {
    ops.iter().find_map(|o| match (o.tag, &o.value) {
        (t, Value::I32x3([x, y, layer])) if t == tag => Some(Whither::Cell((*x, *y, *layer))),
        (t, Value::Name(name) | Value::Str(name)) if t == tag => {
            let name = name.to_ascii_lowercase();
            match name.strip_prefix(CREATURE_POSITION) {
                Some("hero") => Some(Whither::Creature(None)),
                Some(creature) => Some(Whither::Creature(Some(creature.to_owned()))),
                None => named.get(&name).map(|&(x, y)| Whither::Cell((x, y, 0))),
            }
        }
        _ => None,
    })
}

/// What the play's `SetGroupState`s and `DelNPC`s do to the creatures in the world
/// ([`Npcs::regroup`]).
#[derive(Clone, Debug, PartialEq)]
enum Regrouped {
    /// A group's change ([`GroupState`]).
    Group(GroupState),
    /// The creatures of a name removed (`DelNPC`, 55, `0x497F80`).
    Removed(String),
}

/// The death's and sight functions of a `CreateNPC` of the operands `ops`: its `0x05`'s and its
/// `0x60`'s, the latter as `See:` and its name, in their order (`0x483970`, `0x4839CF`; #656,
/// #1025).
fn death_functions(ops: &[opensacred_data::netscript::Operand]) -> DeathFunctions {
    let mut functions = DeathFunctions::default();
    for op in ops {
        match (op.tag, &op.value) {
            (ON_DEATH, Value::Str(name)) => functions.add(name),
            (ON_SIGHT, Value::Str(name)) => functions.see(name),
            _ => {}
        }
    }
    functions
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
            // A share of its most health, the last `0B` held to 0..100 (`0x48E280`, #1001).
            SET_HP => {
                let Some(indices) = name_of(ops).and_then(|name| named.get(&name)) else {
                    continue;
                };
                let share = ops
                    .iter()
                    .filter_map(|o| match (o.tag, &o.value) {
                        (0x0B, Value::I32(v)) => u32::try_from((*v).clamp(0, 100)).ok(),
                        _ => None,
                    })
                    .next_back();
                for &index in indices {
                    if let Some(placement) = placements.get_mut(index) {
                        placement.health_share = share;
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
fn trigger_cells(records: &[&Record]) -> Vec<(String, Spot)> {
    let ops = |kind| {
        records
            .iter()
            .filter(move |r| r.kind == kind)
            .filter_map(|r| match &r.body {
                Body::Operands(ops) => Some(ops),
                _ => None,
            })
    };
    let made = trigger_states(records);
    ops(TRIGGER_PATCH)
        .filter_map(|ops| {
            name_of(ops)
                .filter(|name| made.contains_key(name))
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

/// The state of each trigger that `records`' `CreateTrigger`s make, by its name lower-cased: open
/// (bit 0) and locked (`0x8000`), as the handler takes its markers in order (`0x496520`); a
/// trigger made again takes its new state.
fn trigger_states(records: &[&Record]) -> HashMap<String, (bool, bool)> {
    records
        .iter()
        .filter(|r| r.kind == CREATE_TRIGGER)
        .filter_map(|r| match &r.body {
            Body::Operands(ops) => Some(ops),
            _ => None,
        })
        .filter_map(|ops| {
            let state = ops
                .iter()
                .filter(|o| matches!(o.value, Value::Marker))
                .fold(None, |state, o| {
                    let (open, locked) = state.unwrap_or((false, false));
                    match o.tag {
                        OPEN => Some((true, locked)),
                        CLOSED => Some((false, locked)),
                        LOCKED => Some((false, true)),
                        UNLOCKED => Some((open, false)),
                        _ => state,
                    }
                })?;
            Some((name_of(ops)?, state))
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

/// The texture of every `texture.pak` record (`textures/texture/NNNNN-name.ktx2`), by its number,
/// and of every `texture03.pak` record by `TEXTURE03_BASE` and its number, relative to the asset
/// directory.
pub fn texture_files(assets: &Path) -> Result<HashMap<u32, String>> {
    let mut files = HashMap::new();
    // `texture03.pak`'s after `texture.pak`'s, in the client's one numbering (#1152).
    for (dir, base) in [
        ("texture", 0),
        ("texture03", opensacred_data::texture::TEXTURE03_BASE),
    ] {
        let Ok(entries) = crate::files::read_dir(assets.join("textures").join(dir)) else {
            continue;
        };
        for entry in entries {
            let name = entry
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if let Some(number) = name
                .split_once('-')
                .and_then(|(number, _)| number.parse::<u32>().ok())
                && name.ends_with(&format!(".{}", opensacred_data::texture::EXTENSION))
            {
                files.insert(base + number, format!("textures/{dir}/{name}"));
            }
        }
    }
    Ok(files)
}

/// The `.glb` of every model, by its upper-case `.GRN` name.
pub fn model_files(assets: &Path) -> Result<HashMap<String, String>> {
    let mut files = HashMap::new();
    for dir in ["models", "models03"] {
        let path = assets.join("models").join(dir);
        let Ok(entries) = crate::files::read_dir(&path) else {
            continue;
        };
        for entry in entries {
            let name = entry
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
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

/// What the scripts' records are placed with, at the load and during the play ([`Npcs::read`],
/// [`Npcs::add`]): the asset directory, the items by slot, the weapons' kinds and parents, the
/// models and the textures, and each creature type's speeds, class, fight values, flags and
/// shadow.
struct Makers {
    assets: std::path::PathBuf,
    items: HashMap<u32, Item>,
    kinds: HashMap<u32, u8>,
    /// Each `Weapon.pak` type's parent (`+0x24`), 0 for none: a derived type shows its root's
    /// item ([`Makers::record`]).
    parents: HashMap<u32, u32>,
    models: HashMap<String, String>,
    textures: HashMap<u32, String>,
    speeds: HashMap<u32, [u16; 2]>,
    classes: HashMap<u32, u8>,
    bases: HashMap<u32, fight::Base>,
    record_flags: HashMap<u32, u8>,
    shadowless: HashSet<u32>,
}

/// The places that the scripts' `DefPos`s name, by their names lower-cased: a global cell.
type Places = HashMap<String, (i32, i32)>;

/// What the play's records placed ([`Npcs::add`]): how many creatures and objects, and the
/// `RegionChange`s that the creatures in the world take ([`region_changes`]).
pub struct Added {
    pub placed: usize,
    pub changes: Vec<quests::AreaChange>,
}

/// Where a record ran: in a sector's `Enter`, whose creatures and objects are made again at each
/// coming, and in the start, whose creatures take their levels as the game starts.
#[derive(Clone, Copy, Debug, Default)]
struct Origin {
    enter: bool,
    started: bool,
}

impl Makers {
    /// The placement of the `CreateNPC` or `CreateObj` `record` of the operands `ops`, run where
    /// `origin` tells: its position by the places `named` and the walks' targets `targets`, its
    /// model's clips read into `model_slots`; none where it names no creature, no cell, or a type
    /// without an item or a model.
    fn placement(
        &self,
        record: &Record,
        ops: &[opensacred_data::netscript::Operand],
        origin: Origin,
        (named, targets): (&Places, &HashMap<String, Spot>),
        model_slots: &mut HashMap<String, motions::Slots>,
    ) -> Result<Option<Placement>> {
        let mut items_named = ops.iter().filter_map(|o| match (o.tag, &o.value) {
            (0x02, Value::I32(v)) => u32::try_from(*v).ok(),
            _ => None,
        });
        let creature = items_named.next();
        let items: Vec<u32> = if record.kind == CREATE_NPC {
            items_named.collect()
        } else {
            Vec::new()
        };
        let arming = self.arming(&items);
        // An NPC sent somewhere at the start walks there from where it was made.
        let target = name_of(ops).and_then(|name| targets.get(&name).copied());
        let cell = position(ops, named).or(target);
        let heading = ops.iter().find_map(|o| match (o.tag, &o.value) {
            (0x03, Value::U16(v)) => Some(f32::from(*v)),
            _ => None,
        });
        let (Some(creature), Some((x, y, layer))) = (creature, cell) else {
            return Ok(None);
        };
        let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
            return Ok(None);
        };
        let Some(item) = self.items.get(&creature) else {
            return Ok(None);
        };
        // A fire of the FX objects, which has no model: its variant and its height.
        let fx = (record.kind == CREATE_OBJ && category(item) == FX)
            .then(|| opensacred_rules::particles::fire_variant(creature))
            .flatten()
            .map(|variant| {
                let height = ops.iter().find_map(|o| match (o.tag, &o.value) {
                    (FX_HEIGHT, Value::I32(h)) => i16::try_from(*h).ok().map(f32::from),
                    _ => None,
                });
                (variant, height.unwrap_or(0.0))
            });
        let empty = String::new();
        let Some(model) = self
            .models
            .get(&item.name.to_ascii_uppercase())
            .or(fx.is_some().then_some(&empty))
        else {
            return Ok(None);
        };
        let texture = match item.words_08[0] {
            0 => None,
            number => self.textures.get(&number).cloned(),
        };
        // An NPC's clips standing and walking, by the class of what it holds; walking at a
        // run where its walk is a run.
        let (clips, gait) = if record.kind == CREATE_NPC {
            if !model_slots.contains_key(model) {
                let slots = motions::Slots::read(&self.assets.join(model))?;
                model_slots.insert(model.clone(), slots);
            }
            let slots = &model_slots[model];
            let class = arming.class;
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
                let [walking, running] = self.speeds.get(&creature).copied().unwrap_or_default();
                let speed = if gait == Some(motions::Action::Run) {
                    running
                } else {
                    walking
                };
                (to, f32::from(speed))
            });
        let [walking, running] = self.speeds.get(&creature).copied().unwrap_or_default();
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
        let class = self.classes.get(&creature).copied().unwrap_or_default();
        let hostility = minds::created_hostility(markers());
        let side = minds::created_side(class, creature, word, hostility);
        let trades = if record.kind == CREATE_NPC {
            markers().fold(0, |trades, tag| trades | trade(tag))
        } else {
            0
        };
        Ok(Some(Placement {
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
            base: self.bases.get(&creature).copied().unwrap_or_default(),
            held: arming.class,
            ranged: arming.ranged,
            size: arrows::size(item.words_14[0]),
            made: (origin.enter, true),
            started: origin.started,
            dialogue: record.kind == CREATE_NPC
                && ops.iter().any(|o| {
                    o.tag == DIALOGUE || (o.tag == STAMPED && matches!(o.value, Value::Marker))
                }),
            trades,
            kept: record.kind == CREATE_NPC && ops.iter().any(|o| KEEPS.contains(&o.tag)),
            hire: (record.kind == CREATE_NPC)
                .then(|| hire_of(ops))
                .flatten()
                .filter(|&hire| hire != crate::followers::Hire::LetsGo),
            group: if record.kind == CREATE_NPC {
                script_group(ops)
            } else {
                None
            },
            heading: facing_degrees(record.kind == CREATE_OBJ, heading, item.angle()),
            object: record.kind == CREATE_OBJ,
            casts: if record.kind == CREATE_OBJ {
                category(item) == CHEST
            } else {
                !self.shadowless.contains(&creature)
            },
            blocks: record.kind == CREATE_OBJ && category(item) == CHEST,
            door: record.kind == CREATE_OBJ && category(item) == DOOR,
            fx,
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
                self.record_flags
                    .get(&creature)
                    .copied()
                    .unwrap_or_default(),
                word,
            ),
            side,
            rises: record.kind == CREATE_NPC && rises(ops),
            listed: record.kind == CREATE_NPC && listed(ops),
            hostility,
            texture,
            layer,
            items,
            equipment: arming.equipment,
            holding: arming.holding,
            rank: fight::Rank::of_tag(tag),
            tag,
            clips,
            plain: false,
            name: (record.kind == CREATE_NPC).then(|| name_of(ops)).flatten(),
            health_share: None,
            talk: (record.kind == CREATE_NPC)
                .then(|| {
                    ops.iter().find_map(|o| match (o.tag, &o.value) {
                        (DIALOGUE, Value::Str(name)) => Some(name.clone()),
                        _ => None,
                    })
                })
                .flatten(),
            on_death: if record.kind == CREATE_NPC {
                death_functions(ops)
            } else {
                DeathFunctions::default()
            },
            key: (record.kind == CREATE_OBJ).then(|| name_of(ops)).flatten(),
            lies: record.kind == CREATE_OBJ
                && takeable(category(item))
                && !ops.iter().any(|o| o.tag == QUEST_ITEM),
            quest: (record.kind == CREATE_OBJ
                && takeable(category(item))
                && ops
                    .iter()
                    .any(|o| o.tag == QUEST_ITEM && matches!(o.value, Value::Marker)))
            .then(|| {
                ops.iter().find_map(|o| match (o.tag, &o.value) {
                    (ON_TAKE, Value::Str(name)) => Some(name.clone()),
                    _ => None,
                })
            }),
        }))
    }

    /// The placement `placement` of an NPC whose `CreateNPC` takes the `RegionChange` pair `turn`
    /// as it runs (`0x482F41`, #1063): of the pair's type ([`Self::remade`]), its flags word as
    /// the pair leaves it, and its side by them and its record's hostility, as the record makes a
    /// creature of that type; its items, its place and the rest kept. None where the type has no
    /// item or no model.
    fn retype(
        &self,
        placement: &Placement,
        turn: quests::Turn,
        model_slots: &mut HashMap<String, motions::Slots>,
    ) -> Result<Option<Placement>> {
        let (creature, word) = turn.of(placement.creature.2);
        let Some(mut turned) = self.remade(placement, creature, &placement.items, model_slots)?
        else {
            return Ok(None);
        };
        turned.creature.2 = word;
        turned.side = minds::created_side(turned.class, creature, word, placement.hostility);
        Ok(Some(turned))
    }

    /// The placement `placement` of a creature that a `RegionChange` of its area changes as it
    /// runs (`0x4ABFE0`, #1064): of the type that its pairs turn its type into ([`Self::remade`]),
    /// its items turned with it, the record's flags word and the side it gives; none where no
    /// pair turns its type, or the type has no item or no model. Its values are made anew of the
    /// new type (**hypothesis**: the original keeps the creature's until they are made again).
    fn change(
        &self,
        placement: &Placement,
        change: &quests::AreaChange,
        model_slots: &mut HashMap<String, motions::Slots>,
    ) -> Result<Option<Placement>> {
        let Some((creature, times)) = change.creature(placement.creature.0) else {
            return Ok(None);
        };
        let items: Vec<u32> = placement
            .items
            .iter()
            .map(|&item| change.item(item, times))
            .collect();
        let Some(mut changed) = self.remade(placement, creature, &items, model_slots)? else {
            return Ok(None);
        };
        changed.creature.2 = change.word(placement.creature.2);
        changed.side = change.side(placement.side, creature);
        Ok(Some(changed))
    }

    /// The placement `placement` of an NPC made of the type `creature` holding the items `items`:
    /// its model, texture, size, speeds, `Creature.pak` values, class and flags, its shadow, what
    /// it holds and its clips by them; its flags word, side, place and the rest kept. None where
    /// the type has no item or no model.
    fn remade(
        &self,
        placement: &Placement,
        creature: u32,
        items: &[u32],
        model_slots: &mut HashMap<String, motions::Slots>,
    ) -> Result<Option<Placement>> {
        let Some(item) = self.items.get(&creature) else {
            return Ok(None);
        };
        let Some(model) = self.models.get(&item.name.to_ascii_uppercase()) else {
            return Ok(None);
        };
        if !model_slots.contains_key(model) {
            let slots = motions::Slots::read(&self.assets.join(model))?;
            model_slots.insert(model.clone(), slots);
        }
        let slots = &model_slots[model];
        let arming = self.arming(items);
        let [walking, running] = self.speeds.get(&creature).copied().unwrap_or_default();
        let gait = slots
            .clip(motions::Action::Walk, arming.class)
            .map(|(_, gait)| gait);
        let mut remade = placement.clone();
        remade.model = model.clone();
        remade.texture = match item.words_08[0] {
            0 => None,
            number => self.textures.get(&number).cloned(),
        };
        remade.size = arrows::size(item.words_14[0]);
        remade.speed = [walking, running].map(f32::from);
        if let Some((to, _)) = remade.walk {
            let speed = if gait == Some(motions::Action::Run) {
                running
            } else {
                walking
            };
            remade.walk = Some((to, f32::from(speed)));
        }
        remade.class = self.classes.get(&creature).copied().unwrap_or_default();
        remade.base = self.bases.get(&creature).copied().unwrap_or_default();
        remade.casts = !self.shadowless.contains(&creature);
        remade.clips = ClipNames::of(slots, arming.class);
        remade.held = arming.class;
        remade.ranged = arming.ranged;
        remade.items = items.to_vec();
        remade.equipment = arming.equipment;
        remade.holding = arming.holding;
        remade.creature = (
            creature,
            self.record_flags
                .get(&creature)
                .copied()
                .unwrap_or_default(),
            placement.creature.2,
        );
        Ok(Some(remade))
    }

    /// What an NPC holding the items `items` holds (`0x54ACC0`, `0x54B130`; [`hands`]): their
    /// models in its hands, the items in its hands, whose values it fights with, the class of
    /// what it holds, and whether that is a ranged weapon.
    fn arming(&self, items: &[u32]) -> Arming {
        let equipped: Vec<(u32, &Item)> = items
            .iter()
            .filter_map(|&item| Some((item, self.record(item)?)))
            .collect();
        let categories: Vec<(u8, Option<u8>)> = equipped
            .iter()
            .map(|&(item, record)| (category(record), self.kinds.get(&item).copied()))
            .collect();
        let equipment = equipped
            .iter()
            .zip(hands(&categories))
            .filter_map(|(&(_, item), hand)| {
                let model = self.models.get(&item.name.to_ascii_uppercase())?;
                let texture = match item.words_08[0] {
                    0 => None,
                    number => self.textures.get(&number).cloned(),
                };
                Some(Held {
                    model: model.clone(),
                    texture,
                    hand: hand?,
                    mount: Mount::held(category(item)),
                    slot: None,
                })
            })
            .collect();
        let holding = InHand::of(
            equipped
                .iter()
                .map(|&(item, record)| (item, category(record), self.kinds.get(&item).copied())),
        );
        Arming {
            equipment,
            holding,
            class: held_class(&categories),
            ranged: ranged(&categories),
        }
    }

    /// The `items.pak` record that an item of the type `item` shows: its own, or for a derived
    /// type its nearest ancestor's that has one (`Weapon.pak +0x24`), as [`gear_table`] finds it.
    fn record(&self, item: u32) -> Option<&Item> {
        std::iter::successors(Some(item), |item| {
            self.parents
                .get(item)
                .copied()
                .filter(|&parent| parent != 0)
        })
        .take(8)
        .find_map(|item| self.items.get(&item))
    }

    /// The placement `placement` of a `CreateNPC`'s creature made at the level `level`, each
    /// weapon that its model cannot shoot chosen anew by `armoury` ([`shootable`], #1217), rolled
    /// by `dice`; none where it keeps its items, or its model's clips are not read.
    fn rearmed(
        &self,
        placement: &Placement,
        (level, armoury, dice): (u32, Armoury, &mut spawning::Rng),
        model_slots: &mut HashMap<String, motions::Slots>,
    ) -> Option<Placement> {
        if placement.object || placement.items.is_empty() {
            return None;
        }
        let shots = shots(model_slots.get(&placement.model)?);
        let items = shootable(&placement.items, &self.kinds, shots, |kind| {
            armoury.weapon(kind, level, dice)
        });
        if items == placement.items {
            return None;
        }
        match self.remade(placement, placement.creature.0, &items, model_slots) {
            Ok(remade) => remade,
            Err(e) => {
                bevy::log::warn!("npcs: {e:#}");
                None
            }
        }
    }
}

/// What gives a creature a weapon anew in place of one that its model cannot shoot (#1217): the
/// item types to choose from and the hero as the choice sees it (`0x5C7FA0`).
#[derive(Clone, Copy)]
pub(crate) struct Armoury<'a> {
    catalogue: &'a opensacred_rules::drops::Catalogue,
    hero: &'a opensacred_rules::drops::Hero,
}

impl Armoury<'_> {
    /// A weapon of the kind `kind` at the level `level` as `0x5C7FA0` chooses a creature's: of the
    /// category 5, of the quality 14, of any class and no set piece; none where no type fits.
    fn weapon(&self, kind: u8, level: u32, dice: &mut impl spawning::Dice) -> Option<u32> {
        let wanted = opensacred_rules::drops::Wanted {
            category: WEAPON,
            level,
            class: 0,
            loose: false,
            quality: spawning::REARMED_QUALITY,
            kind: Some(kind),
        };
        self.catalogue.choose(wanted, self.hero, dice)
    }
}

/// The shots of a model of the clips' slots `slots`: whether it has a crossbow's and a bow's.
fn shots(slots: &motions::Slots) -> spawning::Shots {
    spawning::Shots {
        crossbow: slots.slot(spawning::CROSSBOW_SHOT).is_some(),
        bow: slots.slot(spawning::BOW_SHOT).is_some(),
    }
}

/// The items `items` of a `CreateNPC`'s creature, its model of the shots `shots`, each weapon
/// that the model cannot shoot chosen anew by `weapon` of the kind it asks for, in the record's
/// order (`spawning::created_weapon`, `sdk_CREATENPC` at `0x484C44`, #1217): a crossbow or a
/// musket a bow, then a ranged weapon a weapon of kind 1; an item that none is chosen for is left
/// out. `kinds` are the weapons' kinds by their types.
fn shootable(
    items: &[u32],
    kinds: &HashMap<u32, u8>,
    shots: spawning::Shots,
    mut weapon: impl FnMut(u8) -> Option<u32>,
) -> Vec<u32> {
    items
        .iter()
        .filter_map(|&item| {
            let mut item = item;
            for step in [spawning::Step::Crossbow, spawning::Step::Ranged] {
                if let Some(kind) = spawning::created_weapon(step, kinds.get(&item).copied(), shots)
                {
                    item = weapon(kind)?;
                }
            }
            Some(item)
        })
        .collect()
}

/// What an NPC holds ([`Makers::arming`]).
struct Arming {
    equipment: Vec<Held>,
    holding: Vec<InHand>,
    class: u8,
    ranged: bool,
}

impl Npcs {
    /// The level of the sector `sector`, by which the hero's remark at an area's border weighs
    /// it (`0x622D70`): the mean of its `SpawnValues`' lowest and highest, 0 without them.
    pub fn level(&self, sector: (u32, u32)) -> u32 {
        self.values
            .get(&sector)
            .map_or(0, |values| (values.lowest + values.highest) / 2)
    }

    /// Reads the records of the scripts that ran, `tree` (`scripts::run`), and the tables, and
    /// what of them the hero's template's items `worn` are ([`Self::hero_holds`]).
    pub fn read(
        shared: &sectors::Shared,
        tree: &Tree,
        worn: &[u32],
        difficulty: opensacred_rules::drops::Difficulty,
    ) -> Result<Self> {
        let assets = &shared.assets;
        let tables = &shared.tables;
        let items: HashMap<u32, &Item> = tables.items()?.iter().map(|i| (i.slot, i)).collect();
        let mut creatures: Vec<Creature> = tables.creatures()?.to_vec();
        // The waters' crab, which no record describes, made as the original makes a type without
        // one (#664; docs/re/spawning.md, "A type without a record").
        for id in std::iter::once(spawning::CRAB).chain(spawning::FISH) {
            if !creatures.iter().any(|c| c.id == id) {
                creatures.push(opensacred_rules::fight::recordless(id));
            }
        }
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
        let balance = tables.balance().ok();
        // The templates of the items' values (`wpmod.bin`), none where they cannot be read.
        let modifiers: Vec<opensacred_data::tables::WeaponModifier> = files::read_to_string(
            assets
                .join("tables")
                .join(opensacred_data::tables::WEAPON_MODIFIERS),
        )
        .ok()
        .and_then(|text| ron::from_str(&text).ok())
        .unwrap_or_default();
        let weapons = tables.weapons()?;
        // A weapon's kind, by its type, which is its item's slot (docs/02 §6.2).
        let kinds: HashMap<u32, u8> = weapons.iter().map(|w| (w.id, w.bytes_1c[2])).collect();
        let models = model_files(assets)?;
        let textures = texture_files(assets)?;
        let makers = Makers {
            assets: assets.clone(),
            items: items
                .iter()
                .map(|(&slot, &item)| (slot, item.clone()))
                .collect(),
            kinds: kinds.clone(),
            parents: weapons.iter().map(|w| (w.id, w.parent)).collect(),
            models: models.clone(),
            textures: textures.clone(),
            speeds: speeds.clone(),
            classes: classes.clone(),
            bases: bases.clone(),
            record_flags: record_flags.clone(),
            shadowless: shadowless.clone(),
        };
        let records = start_records(tree);
        // The records of the sectors' and the regions' functions, which the start does not run,
        // and those of the sectors' `Enter` functions, whose creatures step otherwise.
        let in_sectors: HashSet<*const Record> = tree
            .functions
            .iter()
            .filter(|f| of_sectors(&f.name))
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
        let positions = positions(records.iter().copied());
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
            let origin = Origin {
                enter: enter.contains(&std::ptr::from_ref(*record)),
                started: !in_sectors.contains(&std::ptr::from_ref(*record)),
            };
            if let Some(placement) = makers.placement(
                record,
                ops,
                origin,
                (&positions, &targets),
                &mut model_slots,
            )? {
                placements.push(placement);
                made.insert(std::ptr::from_ref(*record), placements.len() - 1);
            }
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
        let Sectors { values, fills } = sectors(tree);
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
        // The pools' items, the weapons that an archer takes to melee and back (#396), the
        // hero's (#486), and the bows and the weapons of kind 1 that a crossbow its holder cannot
        // shoot becomes (#1217).
        let gear = gear_table(
            &items,
            weapons,
            &kinds,
            (&models, &textures),
            pools
                .values()
                .flatten()
                .chain(&opensacred_rules::minds::SWITCHED)
                .chain(worn)
                .chain(
                    weapons
                        .iter()
                        .filter(|w| matches!(w.bytes_1c[2], spawning::BOW | spawning::BLADE))
                        .map(|w| &w.id),
                ),
        );
        let side = u32::try_from(SIDE)?;
        let mut by_sector: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (index, placement) in placements.iter().enumerate() {
            let (x, y) = placement.cell;
            by_sector
                .entry((x / side, y / side))
                .or_default()
                .push(index);
        }
        let states = trigger_states(&records);
        let locked: HashSet<String> = states
            .iter()
            .filter(|(_, (_, locked))| *locked)
            .map(|(name, _)| name.clone())
            .collect();
        let opened: HashSet<String> = states
            .iter()
            .filter(|(_, (open, _))| *open)
            .map(|(name, _)| name.clone())
            .collect();
        // The objects' names and their triggers, the last object of a name's (`0x485C10`
        // destroys the older object of a name made again).
        let named: HashMap<String, String> = records
            .iter()
            .filter(|r| r.kind == CREATE_OBJ)
            .filter_map(|r| match &r.body {
                Body::Operands(ops) => {
                    let trigger = ops.iter().find_map(|o| match (o.tag, &o.value) {
                        (TRIGGER, Value::Str(name)) => Some(name.to_ascii_lowercase()),
                        _ => None,
                    })?;
                    Some((name_of(ops)?, trigger))
                }
                _ => None,
            })
            .collect();
        let (dialogues, mut icons) = dialogue_table(&records);
        reicon(&dialogues, &mut icons, set_icons(records.iter().copied()));
        let mut closed: HashMap<(u32, u32), Vec<(String, Spot)>> = HashMap::new();
        let mut triggers: HashMap<String, Vec<Spot>> = HashMap::new();
        for (name, (x, y, floor)) in trigger_cells(&records) {
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
            opened,
            marked: HashSet::new(),
            locked,
            named,
            makers,
            places: positions,
            pending: Vec::new(),
            played: HashSet::new(),
            taken: HashSet::new(),
            dialogues,
            icons,
            region_talks: region_talks(&records),
            values,
            fills,
            templates,
            types,
            classes,
            factors: factors(balance),
            fight: fight::Rules::at(balance, difficulty),
            items: fight::Templates::new(&modifiers, weapons),
            rules: rules(tree),
            pools,
            gear,
            slots: model_slots,
            hits: hit_fractions(tables),
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
            arrows: ARROWS
                .into_iter()
                .filter_map(|kind| {
                    let item = items.get(&kind)?;
                    let model = models.get(&item.name.to_ascii_uppercase())?.clone();
                    let texture = match item.words_08[0] {
                        0 => None,
                        number => textures.get(&number).cloned(),
                    };
                    Some((kind, (model, texture, arrows::size(item.words_14[0]))))
                })
                .collect(),
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
        self.hero_holds_valued(worn, &[])
    }

    /// As [`Self::hero_holds`], the item at each place of `worn` that `valued` gives values of
    /// taking those in place of its record's: an item that the item maker made, put into a weapon
    /// set (#1349).
    pub fn hero_holds_valued(
        &self,
        worn: &[Worn],
        valued: &[Option<fight::ItemValues>],
    ) -> fight::Holds {
        let holding = self.hero_holding(worn);
        let class = class_of(&holding);
        let values = |index: usize, worn: &Worn| fight::ItemValues {
            class: self.gear.get(&worn.item).map_or(0, |gear| gear.category),
            scale: self.items.scale(worn.item),
            ..valued
                .get(index)
                .copied()
                .flatten()
                .unwrap_or_else(|| fight::ItemValues::of_record(&worn.words))
        };
        let in_hands: Vec<(InHand, Option<fight::ItemValues>)> = holding
            .iter()
            .map(|&held| {
                let found = worn
                    .iter()
                    .enumerate()
                    .find(|(_, worn)| worn.item == held.item);
                (held, found.map(|(index, worn)| values(index, worn)))
            })
            .collect();
        fight::Holds {
            weapon: class != 0 || holding.iter().any(|held| held.hand == Hand::Right),
            shield: holding
                .iter()
                .any(|held| held.hand == Hand::Left && held.category == SHIELD),
            ranged: ranged_of(&holding),
            gun: holds_kind(&holding, &GUNS),
            items: worn
                .iter()
                .enumerate()
                .filter(|(_, worn)| self.gear.get(&worn.item).is_none_or(|gear| gear.counts))
                .map(|(index, worn)| values(index, worn))
                .collect(),
            skill: skill_of(class, &in_hands),
        }
    }

    /// The items of `worn`, the hero's template's, that go in its hands, as an NPC's go ([`hands`]).
    fn hero_holding(&self, worn: &[Worn]) -> Vec<InHand> {
        InHand::of(worn.iter().filter_map(|worn| {
            let gear = self.gear.get(&worn.item)?;
            Some((worn.item, gear.category, gear.kind))
        }))
    }

    /// The types of the items of `worn`, the hero's template's, in its left hand and its right:
    /// the halves 0 and 1 of its first weapon set, as the templates keep them (docs/02 §9, #1347).
    pub fn hero_hands(&self, worn: &[Worn]) -> [Option<u32>; 2] {
        let holding = self.hero_holding(worn);
        let in_hand = |hand| {
            holding
                .iter()
                .find(|held| held.hand == hand)
                .map(|held| held.item)
        };
        [in_hand(Hand::Left), in_hand(Hand::Right)]
    }

    /// The items of `worn`, the hero's template's, but those in its hands: what it wears while
    /// its hands' items are put away (the Seraphim's 1018, `arsenal`).
    pub fn hero_put_away(&self, worn: &[Worn]) -> Vec<Worn> {
        let in_hands: Vec<u32> = self
            .hero_holding(worn)
            .iter()
            .map(|held| held.item)
            .collect();
        worn.iter()
            .filter(|worn| !in_hands.contains(&worn.item))
            .cloned()
            .collect()
    }

    /// The item of `worn`, its template's items, that the Gladiator's 1013 hurls (`0x501D39`):
    /// the weapon in its left hand, else the one in its right (`0x427A40` admits the category
    /// 5 alone), its record and its model; and the rest of `worn`.
    pub fn hero_hurls(&self, worn: &[Worn]) -> Option<(Worn, Held, Vec<Worn>)> {
        let holding = self.hero_holding(worn);
        let weapon = |hand| {
            holding
                .iter()
                .find(|held| held.hand == hand && held.category == WEAPON)
        };
        let held = weapon(Hand::Left).or_else(|| weapon(Hand::Right))?;
        let gear = self.gear.get(&held.item)?;
        let model = Held {
            model: gear.model.clone()?,
            texture: gear.texture.clone(),
            hand: held.hand,
            mount: Mount::Hand,
            slot: None,
        };
        let at = worn.iter().position(|worn| worn.item == held.item)?;
        let mut rest = worn.to_vec();
        let thrown = rest.remove(at);
        Some((thrown, model, rest))
    }

    /// What the hero holds of `worn`, its template's items, for its weapons' sounds
    /// ([`Self::armed`]).
    pub fn hero_armed(&self, worn: &[Worn]) -> crate::sfx::Armed {
        self.armed(&self.hero_holding(worn))
    }

    /// The item of `worn`, its template's items, that the hero's hurled blades strike with
    /// (`0x5038F0`): the one in its right hand, else the one in its left, and its size.
    pub fn hero_blade(&self, worn: &[Worn]) -> Option<crate::making::Blade> {
        let holding = self.hero_holding(worn);
        let held = holding
            .iter()
            .find(|held| held.hand == Hand::Right)
            .or_else(|| holding.iter().find(|held| held.hand == Hand::Left))?;
        Some(crate::making::Blade {
            right: held.hand == Hand::Right,
            size: self.gear.get(&held.item)?.size,
        })
    }

    /// What the items `holding` give for a striker's weapons' sounds: the class of what it holds
    /// and the sound profile of the weapon in its right hand, else of the one in its left that is
    /// no shield, none for bare hands.
    fn armed(&self, holding: &[InHand]) -> crate::sfx::Armed {
        let weapon = holding
            .iter()
            .find(|held| held.hand == Hand::Right)
            .or_else(|| {
                holding
                    .iter()
                    .find(|held| held.hand == Hand::Left && held.category != SHIELD)
            })
            .map(|held| {
                (
                    held.item,
                    self.gear.get(&held.item).map_or(0, |gear| gear.sound),
                )
            });
        crate::sfx::Armed {
            class: class_of(holding),
            weapon: weapon.map(|(_, sound)| sound),
            kind: weapon.map(|(kind, _)| kind),
        }
    }

    /// The class of what the hero holds of `worn`, its template's items ([`class_of`]), by which
    /// it picks its clips, and the models of those in its hands (#488) and of those it wears on
    /// its skeleton (#1131).
    pub fn hero_gear(&self, worn: &[Worn]) -> (u8, Vec<Held>) {
        let holding = self.hero_holding(worn);
        let mut held: Vec<Held> = holding
            .iter()
            .filter_map(|held| {
                let gear = self.gear.get(&held.item)?;
                Some(Held {
                    model: gear.model.clone()?,
                    texture: gear.texture.clone(),
                    hand: held.hand,
                    mount: Mount::held(held.category),
                    slot: None,
                })
            })
            .collect();
        held.extend(worn_gear(&self.gear, worn, &holding));
        (class_of(&holding), held)
    }

    /// The model and texture of the arrows of type `kind`, relative to the asset directory, and
    /// their size, where their model was converted.
    pub fn arrow(&self, kind: u32) -> Option<(&str, Option<&str>, f32)> {
        self.arrows
            .get(&kind)
            .map(|(model, texture, size)| (model.as_str(), texture.as_deref(), *size))
    }

    /// The NPCs and the objects to place, and the cells of the closed triggers.
    pub fn counts(&self) -> (usize, usize, usize) {
        let objects = self.placements.iter().filter(|p| p.object).count();
        let closed = self
            .closed
            .values()
            .flatten()
            .filter(|(name, _)| !self.opened.contains(name))
            .count();
        (self.placements.len() - objects, objects, closed)
    }
}

/// The gear of the items of the types `slots`, which are their `items.pak` slots: a fixed item,
/// a record of `Weapon.pak` without an `items.pak` slot of its own, as the heroes' templates' are,
/// takes its parent's (`+0x24`), its model and its category, as the client copies the parent's
/// definition into the child (docs/02 §6.2; #488).
fn gear_table<'a>(
    items: &HashMap<u32, &Item>,
    weapons: &[opensacred_data::tables::Weapon],
    kinds: &HashMap<u32, u8>,
    (models, textures): (&HashMap<String, String>, &HashMap<u32, String>),
    slots: impl IntoIterator<Item = &'a u32>,
) -> HashMap<u32, Gear> {
    let parents: HashMap<u32, u32> = weapons.iter().map(|w| (w.id, w.parent)).collect();
    let record = |slot: u32| {
        std::iter::successors(Some(slot), |slot| {
            parents.get(slot).copied().filter(|&parent| parent != 0)
        })
        .take(8)
        .find_map(|slot| items.get(&slot).copied())
    };
    slots
        .into_iter()
        .filter_map(|&slot| {
            let item = record(slot)?;
            let gear = Gear {
                category: category(item),
                kind: kinds.get(&slot).copied(),
                model: models.get(&item.name.to_ascii_uppercase()).cloned(),
                texture: match item.words_08[0] {
                    0 | FX_OPAQUE => None,
                    number => textures.get(&number).cloned(),
                },
                counts: item.byte_03 & 2 == 0,
                sound: items.get(&slot).map_or(0, |own| own.sound()),
                size: arrows::size(item.words_14[0]),
            };
            Some((slot, gear))
        })
        .collect()
}

/// A hero's slots of its hands and its back (`0x401AE0`): the right hand's, the left hand's and
/// the back's.
const RIGHT_HAND: u8 = 13;
const LEFT_HAND: u8 = 12;
const BACK: u8 = 17;

/// What a figure of the choice of the hero shows of the items `slots`, each item's type in a slot
/// of a hero's (`choice`, #1130): the class of what is in its hands, by which it picks its clip,
/// and the items' models ([`showcase`]); `files` are the models' and the textures'
/// ([`model_files`], [`texture_files`]).
pub fn figure_gear(
    tables: &crate::tables::Tables,
    files: (&HashMap<String, String>, &HashMap<u32, String>),
    slots: &[(u8, u32)],
) -> Result<(u8, Vec<Held>)> {
    let items: HashMap<u32, &Item> = tables.items()?.iter().map(|i| (i.slot, i)).collect();
    let weapons = tables.weapons()?;
    let kinds: HashMap<u32, u8> = weapons.iter().map(|w| (w.id, w.bytes_1c[2])).collect();
    let types: Vec<u32> = slots.iter().map(|&(_, item)| item).collect();
    let gear = gear_table(&items, weapons, &kinds, files, &types);
    Ok(showcase(&gear, slots))
}

/// The items `slots` on a figure of the choice of the hero, as `cUI_NetGranny` puts them on
/// (`0x6F3E90`; docs/re/equipment.md, "The choice of the hero's figures"): the slot 13's in its
/// right hand, the slot 12's in its left, on its forearm for a shield, the slot 17's on its back,
/// and those of the worn slots 0 to 7 and 14 to 16 on its skeleton; the class of what is in its
/// hands, and the models.
fn showcase(gear: &HashMap<u32, Gear>, slots: &[(u8, u32)]) -> (u8, Vec<Held>) {
    let mut holding = Vec::new();
    let mut held = Vec::new();
    for &(slot, item) in slots {
        let Some(gear) = gear.get(&item) else {
            continue;
        };
        let (hand, mount) = match slot {
            RIGHT_HAND => (Hand::Right, Mount::Hand),
            LEFT_HAND => (Hand::Left, Mount::held(gear.category)),
            BACK => (Hand::Right, Mount::Back),
            0..=7 | 14..=16 => (Hand::Right, Mount::Worn),
            _ => continue,
        };
        if mount.handed() {
            holding.push(InHand {
                item,
                category: gear.category,
                kind: gear.kind,
                hand,
            });
        }
        if let Some(model) = gear.model.clone() {
            held.push(Held {
                model,
                texture: gear.texture.clone(),
                hand,
                mount,
                slot: (!mount.handed()).then_some(slot),
            });
        }
    }
    (class_of(&holding), held)
}

/// The items of `worn` that their wearer wears on its skeleton: those not in its hands
/// (`holding`) whose categories it does not carry on a bone ([`ATTACHED`]), each with its model
/// and texture (#1131), in the first slot that takes its category (#1307). A template's are the class's own cosmetic items, which its default
/// (`0x426580`) also gives: the Seraphim's hair, the Gladiator's belt, the Battle Mage's hood, the
/// Vampiress's hair and the Dwarf's goggles (docs/re/equipment.md).
fn worn_gear(gear: &HashMap<u32, Gear>, worn: &[Worn], holding: &[InHand]) -> Vec<Held> {
    worn.iter()
        .filter(|worn| !holding.iter().any(|held| held.item == worn.item))
        .filter_map(|worn| {
            let gear = gear.get(&worn.item)?;
            if ATTACHED.contains(&gear.category) {
                return None;
            }
            Some(Held {
                model: gear.model.clone()?,
                texture: gear.texture.clone(),
                hand: Hand::Right,
                mount: Mount::Worn,
                slot: opensacred_rules::equipment::slot_for(gear.category, |_| false),
            })
        })
        .collect()
}

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
    /// Hostile or peaceful, else by their class's side: hostile where it is hostile to the heroes
    /// (`minds::hostile_to_heroes`), as the creature maker makes them for neither.
    pub hostile: Option<bool>,
    /// Whether they take no damage (`fight::Immortal`).
    pub immortal: bool,
    /// Whether no sector keeps them: they go with the game alone, as the hero's followers do
    /// (`followers`), wherever it takes them.
    pub roams: bool,
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

    /// The middle of the levels' range of the sector of global cell `cell`, where its record
    /// bounds them (`sdk_HIRE`'s level of a follower).
    pub fn middle(&self, (x, y): (u32, u32)) -> Option<u32> {
        let side = u32::try_from(SIDE).unwrap_or(64);
        self.values
            .get(&(x / side, y / side))
            .filter(|values| values.highest != u32::MAX)
            .map(|values| (values.lowest + values.highest) / 2)
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
    // Hostile or peaceful by its class's side, as the creature maker makes it for neither
    // (`0x480D40`'s −1 asks `0x549000` whether it is hostile to the local hero, at `0x480FD5`).
    let hostile = request
        .hostile
        .unwrap_or_else(|| minds::hostile_to_heroes(placement.class, creature));
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
        // A level asked for is the creature's own, not a boss's marker to level at its first
        // fight.
        if request.level.is_some() {
            commands.entity(entity).remove::<fight::Unfought>();
        }
        if request.roams {
            commands
                .entity(entity)
                .insert(bevy::prelude::DespawnOnExit(crate::game::AppState::InGame));
            made.push(entity);
            continue;
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

/// Places the placement `index` of the sector `at` as it comes in: a kept one anew the first time,
/// where it stood after and never once killed, the others anew each time. Made anew, a weapon
/// that its model cannot shoot is chosen anew by `armoury` where it is read (#1217), which a kept
/// one keeps.
fn place_index(
    commands: &mut Commands,
    npcs: &mut Npcs,
    (index, at): (usize, (u32, u32)),
    (heights, states, assets): (&Heights, &buildings::Buildings, &AssetServer),
    (hero, minutes, area_types): (u32, u32, Option<&quests::AreaTypes>),
    armoury: Option<Armoury>,
    streamer: &mut Streamer,
) {
    // A quest's item that the hero took lies no more (#1256).
    if npcs.taken.contains(&index) {
        return;
    }
    // An `Enter`'s creature, made again as its `CreateNPC` runs again, of the type that its
    // area's `RegionChange` list gives it now (`0x482F41`, #1063).
    let turn = area_types
        .filter(|_| npcs.placements[index].made.0 && npcs.placements[index].turns())
        .zip(streamer.area(at))
        .and_then(|(lists, area)| lists.turn(area, npcs.placements[index].creature.0));
    let turned = match turn {
        Some(turn) => match npcs
            .makers
            .retype(&npcs.placements[index], turn, &mut npcs.slots)
        {
            Ok(Some(turned)) => Some(turned),
            Ok(None) => return,
            Err(e) => {
                bevy::log::warn!("npcs: {e:#}");
                return;
            }
        },
        None => None,
    };
    let placement = turned.as_ref().unwrap_or(&npcs.placements[index]);
    // The start's and the `Init`s' creatures persist; the `Enter`s' are made again.
    let kept = !placement.object && !placement.made.0;
    let again = if kept {
        npcs.kept.again(index)
    } else {
        Again::Anew
    };
    // Made anew, the level its record makes it at: the start's creature's as the game starts,
    // with the hero of its first level and no time played; a sector's as its function runs.
    let made_at = matches!(again, Again::Anew).then(|| {
        if placement.started {
            npcs.scripted_level(placement, fight::HERO_LEVEL, 0)
        } else {
            npcs.scripted_level(placement, hero, minutes)
        }
    });
    // A weapon that its model cannot shoot chosen anew at that level (`sdk_CREATENPC` at
    // `0x484C44`, #1217), the same each time the same record makes it.
    let rearmed = made_at.zip(armoury).and_then(|(level, armoury)| {
        let mut dice = spawning::Rng::of(placement.cell.0, placement.cell.1, index);
        npcs.makers
            .rearmed(placement, (level, armoury, &mut dice), &mut npcs.slots)
    });
    let placement = rearmed.as_ref().unwrap_or(placement);
    let entity = match again {
        Again::Not => return,
        Again::Anew => {
            // A script's boss then at the hero's level now and more, as a fill's (`0x550E50`).
            let level = made_at.unwrap_or_default();
            let values = npcs.values.get(&at).copied().unwrap_or_default();
            let big = placement.creature.1 & crowd::BIG != 0;
            let level = placement.rank.level(level, hero, &values, big, &npcs.fight);
            spawn(
                commands,
                placement,
                heights,
                states,
                assets,
                (level, &npcs.fight, &npcs.items),
            )
        }
        Again::AtStood(stood) => respawn(
            commands,
            placement,
            stood,
            (heights, states, assets),
            (&npcs.fight, &npcs.items),
        ),
    };
    // A quest's item lies to be picked up as a drop, its taking's function with it (#1256).
    if let Some(take) = &placement.quest {
        commands.entity(entity).insert((
            QuestItem {
                take: take.clone(),
                index,
                name: placement.key.clone(),
            },
            drops::Lying {
                dropped: opensacred_rules::drops::Dropped {
                    kind: opensacred_rules::drops::Kind::Other,
                    item: placement.creature.0,
                    level: 0,
                    key: QUEST_KEY,
                    quality: 0,
                    amount: 1,
                    values: None,
                },
                resting: true,
            },
        ));
    }
    // An item of a `CreateObj` lies to be picked up as a drop (#1296).
    if placement.lies && placement.quest.is_none() {
        commands.entity(entity).insert((
            ScriptItem { index },
            drops::Lying {
                dropped: opensacred_rules::drops::Dropped {
                    kind: opensacred_rules::drops::Kind::Other,
                    item: placement.creature.0,
                    level: 0,
                    key: QUEST_KEY,
                    quality: 0,
                    amount: 1,
                    values: None,
                },
                resting: true,
            },
        ));
    }
    // A lying item of a `res:` name goes by its text (#1360).
    if (placement.lies || placement.quest.is_some())
        && let Some(name) = placement.key.as_deref().and_then(TextName::of)
    {
        commands.entity(entity).insert(name);
    }
    // An `Enter`'s creature keeps what it was made of, which a `RegionChange` changes (#1064).
    if placement.made.0 && !placement.object {
        commands
            .entity(entity)
            .insert(Remakes(Box::new(placement.clone())));
    }
    // A door whose trigger is open, as it was left or as the scripts set it while it was
    // gone, opens as it comes (#952).
    if placement
        .trigger
        .as_ref()
        .is_some_and(|trigger| npcs.opened.contains(trigger))
    {
        commands.entity(entity).insert(Swing::by_script(true));
    }
    if kept {
        let placed = Placed {
            at,
            source: Source::Kept(index),
        };
        npcs.kept.spawned(&placed);
        commands.entity(entity).insert(placed);
    }
    streamer.adopt(at, entity);
    // A kept creature comes again with the weapon chosen for it (`respawn`).
    if kept
        && turned.is_none()
        && let Some(rearmed) = rearmed
    {
        npcs.placements[index] = rearmed;
    }
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
    (played, replacements, area_types): (
        Res<drops::Played>,
        Option<Res<crate::scripts::ScriptReplacements>>,
        Option<Res<crate::scripts::ScriptAreaTypes>>,
    ),
    items: Option<Res<drops::Tables>>,
    bodies: Query<&fight::Base, With<hero::Body>>,
) {
    let npcs = &mut *npcs;
    let hero = experience.level;
    let minutes = played.minutes();
    let area_types = area_types.as_ref().map(|lists| &lists.0);
    let side = u32::try_from(SIDE).unwrap_or(64);
    // The weapons' choice, once the drops' tables are read, for the hero, or one of no class
    // before it stands in the world (#1217).
    let chooser = bodies.iter().next().map_or(
        opensacred_rules::drops::Hero {
            creature: 0,
            level: hero,
            ..opensacred_rules::drops::Hero::default()
        },
        |base| drops::hero(base, hero),
    );
    let armoury = items.as_ref().map(|items| Armoury {
        catalogue: items.catalogue(),
        hero: &chooser,
    });
    // The placements that the play's scripts made in the sectors in the world (#1014).
    for index in std::mem::take(&mut npcs.pending) {
        let (x, y) = npcs.placements[index].cell;
        place_index(
            &mut commands,
            npcs,
            (index, (x / side, y / side)),
            (&heights, &states, &assets),
            (hero, minutes, area_types),
            armoury,
            &mut streamer,
        );
    }
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
        let indices = npcs.by_sector.get(&at).cloned().unwrap_or_default();
        for index in indices {
            place_index(
                &mut commands,
                npcs,
                (index, at),
                (&heights, &states, &assets),
                (hero, minutes, area_types),
                armoury,
                &mut streamer,
            );
        }
        // The fills' creatures that went with the sector come back where they stood, or it fills
        // anew, by the region that filled last (#1107).
        let filling = npcs
            .fills
            .get(&at)
            .is_some_and(|fills| !fills.is_empty())
            .then(|| quests::region(streamer.area(at).unwrap_or(0)));
        let entered = npcs.kept.enter(at, filling, |sector| {
            quests::region(streamer.area(sector).unwrap_or(0))
        });
        bevy::log::debug!(
            "npcs: {at:?} comes in, {} of its fills back, filling {}",
            entered.back.len(),
            if entered.fill { "anew" } else { "nothing" }
        );
        // The crabs and fish of its waters where none comes back (`0x4939F0`), once the sectors
        // about it are in.
        if entered.back.is_empty() && !npcs.waters.contains(&at) {
            npcs.waters.push(at);
        }
        for (placement, stood) in entered.back {
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
        if !entered.fill {
            continue;
        }
        // The creatures that the sector's fills make, on cells at random that they may stand
        // on, the same ones each time the sector comes in (`spawning`).
        let Some(fills) = npcs.fills.get(&at) else {
            continue;
        };
        // As the scripts' `ReplaceSpawn`s of its area change them (#55).
        let area = streamer.area(at).unwrap_or(0);
        let fills: Vec<spawning::Fill> = match &replacements {
            Some(replacements) if !replacements.0.is_empty() => fills
                .iter()
                .map(|fill| replacements.0.fill(area, fill))
                .collect(),
            _ => fills.clone(),
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
            let shared = fight::fill_level(&values, hero, minutes, &npcs.fight, &mut rng);
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
                // Its items by the sector's rules, in its hands, and its clips by what it holds; a
                // crossbow its model cannot shoot another weapon of the run's level (#1217).
                let rules = npcs.rules.get(&at).map(Vec::as_slice).unwrap_or_default();
                let shots = match (armoury, npcs.slots.get(&placement.model)) {
                    (Some(_), Some(slots)) => shots(slots),
                    _ => spawning::Shots {
                        crossbow: true,
                        bow: true,
                    },
                };
                let weapon = |kind, rng: &mut spawning::Rng| {
                    armoury.and_then(|armoury| armoury.weapon(kind, shared, rng))
                };
                let gear = gear_of(
                    rules,
                    made.creature,
                    (&npcs.pools, &npcs.gear),
                    (shots, weapon),
                    &mut rng,
                );
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
                                mount: Mount::held(g.category),
                                slot: None,
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

/// Spawns the crabs and fish of the waters of sector `at`, come in with no creatures kept
/// (`0x4939F0`, `spawning::swimmers`): of its liquid materials, its area and whether it is a dungeon, the
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
        // Not kept as it goes, lacking the fills' mark (`+0x2B8` bit `0x80`): made again at the
        // next coming in where none comes back.
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
/// kept one's place, floor and health, or that it was killed, and the death's functions it still
/// has (#656); a fill's living one, with its sector.
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
        Option<&OnDeath>,
        Has<Remade>,
    )>,
    heights: Option<Res<Heights>>,
    npcs: Option<ResMut<Npcs>>,
) {
    let (Ok((placed, transform, way, health, dies, on_death, remade)), Some(mut npcs)) =
        (creatures.get(removed.entity), npcs)
    else {
        return;
    };
    // A creature made again by a `RegionChange` leaves its place to its new self (#1064).
    if remade {
        return;
    }
    if let Source::Kept(index) = placed.source
        && let Some(placement) = npcs.placements.get_mut(index)
    {
        placement.on_death = on_death
            .map(|on_death| on_death.0.clone())
            .unwrap_or_default();
    }
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
    if let Some((variant, height)) = placement.fx {
        return crate::fx::spawn(
            commands,
            variant,
            Vec3::new(model.x, height * iso::MODEL_SCALE, model.z),
            (layered, layered.visibility(states)),
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
                kind: placement.creature.0,
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
        mind.hold(placement.dialogue, placement.trades != 0, placement.kept);
        if placement.trades != 0 {
            entity.insert(Trades(placement.trades));
        }
        if let Some(hire) = placement.hire {
            entity.insert(crate::followers::Hired(hire));
        }
        mind.side = placement.side;
        // The category `0x20`'s, which the original draws under the water too (`0x434070`).
        if minds::still(creature) {
            entity.insert((Still, models::Sinks));
        }
        bevy::log::debug!(
            "npcs: {} is a {creature} of the flags {word:#x}, {:#x} after its first update, side \
             {}, group {:?}",
            entity.id(),
            mind.flags,
            mind.side,
            mind.group
        );
        mind.level = grade;
        mind.ranged = placement.ranged;
        mind.armed = placement.held != 0;
        mind.big = record_flags & BIG != 0;
        // Its health, full ([`most_health`]).
        let skills = fight::skills(&placement.base, grade, placement.plain);
        let most = most_health(placement, grade, rules);
        // A `SetHP`'s share of it (`0x48E280`, #1001), truncated.
        let current = placement
            .health_share
            .map_or(most, |share| most * share / 100);
        entity.insert(fight::Health {
            current,
            most,
            level: grade,
        });
        if let Some(name) = &placement.name {
            entity.insert(ScriptName(name.clone()));
        }
        if !placement.on_death.is_empty() {
            entity.insert(OnDeath(placement.on_death.clone()));
        }
        if let Some(talk) = &placement.talk {
            entity.insert(Talk(talk.clone()));
        }
        // Its attack, defence, damage and resistances with the values of its items, made at its
        // level (`0x5D5080`), the same each time it is made, its hits' share, and its
        // regeneration.
        let mut dice = spawning::Rng::of(x, y, usize::try_from(creature).unwrap_or(0));
        // A creature hostile to the hero hardened on platinum and niobium (`0x5796A0`, #1125).
        let base = fight::Base(opensacred_rules::fight::Base {
            hardened: rules.hardened(minds::hostile_to_heroes(placement.side, creature)),
            ..placement.base.0
        });
        let arms = Arms {
            base,
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
            fight::Regenerates::new(fight::regeneration(&base, grade, &skills, rules)),
        ));
        // A boss or a champion not hostile to the hero takes its level at its first fight.
        if placement.rank != fight::Rank::Common
            && !minds::hostile_to_heroes(placement.side, creature)
        {
            entity.insert(fight::Unfought {
                big: record_flags & crowd::BIG != 0,
            });
        }
        entity.insert((
            Npc {
                gltf: assets.load(placement.model.clone()),
                model: placement.model.clone(),
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
            entity.insert(
                Walk::to(
                    iso::ground(tx as f32 + 0.5, ty as f32 + 0.5),
                    level,
                    speed * pace,
                )
                .idle(),
            );
        }
    }
    entity.id()
}

/// The most health of a creature of `placement` at the level `grade`, by its base, its level, its
/// skill 10 and its rank (`0x5658F0`); a crab's or a fish's the stats' own.
fn most_health(placement: &Placement, grade: u32, rules: &fight::Rules) -> u32 {
    if placement.plain {
        return PLAIN_HEALTH;
    }
    let skills = fight::skills(&placement.base, grade, placement.plain);
    let rank = placement
        .rank
        .health(placement.creature.0, placement.class, rules);
    fight::most_health(placement.base.attributes, grade, &skills, rank, rules)
}

/// The health `health` as the share of it that a creature of `placement` keeps, its most made
/// anew of its level ([`most_health`]): a changed creature's (#1064).
fn health_kept(
    health: fight::Health,
    placement: &Placement,
    rules: &fight::Rules,
) -> fight::Health {
    let most = most_health(placement, health.level, rules);
    let current = u64::from(most) * u64::from(health.current.min(health.most))
        / u64::from(health.most.max(1));
    fight::Health {
        current: u32::try_from(current).unwrap_or(most),
        most,
        level: health.level,
    }
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

/// The model of an item that a creature wears on its skeleton (`equip`): its wearer, whose bones
/// its skinned meshes follow ([`bind_worn`]).
#[derive(Component, Clone, Copy, Debug)]
pub struct WornBy(pub Entity);

/// A worn item's skinned mesh whose joints are its wearer's bones.
#[derive(Component)]
pub struct Bound;

/// The model of an item that a creature carries on its back (`equip`, [`Mount::Back`]).
#[derive(Component, Clone, Copy, Debug)]
pub struct Shouldered;

/// An item's model on a creature, held in a hand ([`Wielded`]), carried on its back
/// ([`Shouldered`]) or worn ([`WornBy`]).
pub type Carried = Or<(With<Wielded>, With<Shouldered>, With<WornBy>)>;

/// Binds the skinned meshes of each worn item to its wearer's bones of their joints' names, as
/// `GrannyWearSequence` merges a worn item onto its wearer's skeleton (`0x44C980`, docs/re/
/// equipment.md, "Worn items and the classes' defaults"): each joint becomes the wearer's bone of
/// its name where the wearer has one, not a bone of the items it holds or wears.
pub fn bind_worn(
    mut commands: Commands,
    worn: Query<(Entity, &WornBy)>,
    children: Query<&Children>,
    names: Query<&Name>,
    items: Query<(), Carried>,
    mut meshes: Query<&mut SkinnedMesh, Without<Bound>>,
) {
    for (piece, by) in &worn {
        let skinned: Vec<Entity> = children
            .iter_descendants(piece)
            .filter(|&entity| meshes.contains(entity))
            .collect();
        if skinned.is_empty() {
            continue;
        }
        let mut bones: HashMap<&str, Entity> = HashMap::new();
        let mut stack = vec![by.0];
        while let Some(entity) = stack.pop() {
            if let Ok(name) = names.get(entity) {
                bones.entry(name.as_str()).or_insert(entity);
            }
            for &child in children.get(entity).into_iter().flatten() {
                if !items.contains(child) {
                    stack.push(child);
                }
            }
        }
        for entity in skinned {
            let Ok(mut mesh) = meshes.get_mut(entity) else {
                continue;
            };
            for joint in &mut mesh.joints {
                let bone = names
                    .get(*joint)
                    .ok()
                    .and_then(|name| bones.get(name.as_str()));
                if let Some(&bone) = bone {
                    *joint = bone;
                }
            }
            commands.entity(entity).insert((Bound, NoFrustumCulling));
        }
    }
}

/// An NPC's equipment still to attach: each item's model, its texture, its hand, and whether the
/// NPC's own scene is ready.
#[derive(Component)]
pub struct Equipment(Vec<Equip>);

pub struct Equip {
    gltf: Handle<Gltf>,
    texture: Option<Handle<Image>>,
    hand: Hand,
    mount: Mount,
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
/// their hands), else its hand, and for a shield always its forearm; for the back, the item's
/// `Bone_spine` on the holder's `Bip01 Spine2` (`0x6F3E90`).
fn bones(hand: Hand, mount: Mount) -> (&'static str, Option<&'static str>, &'static str) {
    match (hand, mount) {
        (_, Mount::Back) => ("Bone_spine", None, "Bip01 Spine2"),
        (Hand::Left, Mount::Forearm) => ("Bone_weapon_02", None, "Bip01 L Forearm"),
        (Hand::Right, Mount::Forearm) => ("Bone_weapon_01", None, "Bip01 R Forearm"),
        (Hand::Left, _) => ("Bone_weapon_02", Some("Bone_weapon_02"), "Bip01 L Hand"),
        (Hand::Right, _) => ("Bone_weapon_01", Some("Bone_weapon_01"), "Bip01 R Hand"),
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
                mount: held.mount,
                ready: false,
                waits: SETTLES,
            })
            .collect(),
    )
}

impl Equipment {
    /// The equipment ready to attach at once: its holder's scene is in place already, as a hero's
    /// whose form keeps its model (`hero::Hero::wear`).
    pub fn ready(mut self) -> Self {
        for item in &mut self.0 {
            item.ready = true;
        }
        self
    }
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
            // A worn item's model goes under its wearer, its skinned meshes then bound to the
            // wearer's bones ([`bind_worn`]).
            if item.mount == Mount::Worn {
                let Some(scene) = gltf.scenes.first().cloned() else {
                    return false;
                };
                if !worlds.contains(&scene) {
                    return true;
                }
                let mut piece = commands.spawn((
                    WorldAssetRoot(scene),
                    Transform::default(),
                    ChildOf(npc),
                    models::OrderedWith(npc),
                    WornBy(npc),
                ));
                if let Some(texture) = &item.texture {
                    piece.insert(Retexture(texture.clone()));
                }
                bevy::log::debug!("npcs: {npc} wears {}", piece.id());
                return false;
            }
            let (socket, own, holder) = bones(item.hand, item.mount);
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
            ));
            if item.mount == Mount::Back {
                weapon.insert(Shouldered);
            } else {
                weapon.insert(Wielded(item.hand));
            }
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

/// A node's place in its model by the chain of nodes from the root: the whole of their
/// transforms, scales included, and the product of their rotations alone.
#[derive(Clone, Copy)]
struct Chain {
    whole: Affine3A,
    rotation: Quat,
}

impl Chain {
    const ROOT: Self = Self {
        whole: Affine3A::IDENTITY,
        rotation: Quat::IDENTITY,
    };

    /// The place of a child whose own transform is `local`.
    fn then(self, local: &Transform) -> Self {
        Self {
            whole: self.whole * local.compute_affine(),
            rotation: self.rotation * local.rotation,
        }
    }

    /// The node's position and orientation, without a scale: the position by the whole chain,
    /// so that the scales above the node place it, and the orientation by the rotations.
    fn pose(self) -> Transform {
        Transform::from_translation(self.whole.translation.into()).with_rotation(self.rotation)
    }
}

/// The pose of the node named `name` in a glTF's scene, from its root: its position and
/// orientation alone ([`Chain::pose`]). The scale of the sockets takes no part: the dark elves'
/// blades have sockets of scale 0.23 and meshes as long as a sword's (**hypothesis**: the
/// client's attach takes the components of mask `0x3F`, `0x44C670`, which by these models are
/// the position and the orientation). The scales of the nodes above a socket do place it: the
/// daggers' `Bone_weapon_01` lies under a node of scale 0.4, and placed without it they were held
/// a hand's width off their grip (#814). In every item the socket's whole transform is a rotation
/// times a uniform scale, the rotation that the product of the chain's rotations gives.
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
    let mut stack: Vec<(&GltfNode, Chain)> = all
        .into_iter()
        .filter(|node| !children.contains(&node.index))
        .map(|node| (node, Chain::ROOT))
        .collect();
    while let Some((node, parent)) = stack.pop() {
        let placed = parent.then(&node.transform);
        if node.name == name {
            return Some(placed.pose());
        }
        stack.extend(
            node.children
                .iter()
                .filter_map(|child| nodes.get(child))
                .map(|child| (child, placed)),
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
                scripted: HashMap::new(),
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
    // Who stands in the way, and who stays where it is made.
    (crowd, still): (Res<crowd::Crowd>, Query<(), With<Still>>),
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
        if still.contains(entity) {
            commands.entity(entity).remove::<Walk>();
            continue;
        }
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
        let onto = walk.onto;
        // A cell it may step into: walkable, no object and no other creature in it (`0x4F7E10`:
        // an NPC, of an id above 16, is stopped by any).
        let free =
            |cell, at| heights.walkable_on(cell, at) && !crowd.stops(cell, at, entity, false);
        // Its leg and its course there, found as it sets out on the leg, anew off it (a stair's
        // landing) and at the leg's end short of its target.
        let plan = |here: Vec3| {
            let leg = paths::leg(here, target, |cell| heights.walkable_on(cell, level));
            let end = paths::cell_of(leg.x, leg.z);
            let course = paths::course(
                here,
                (level, false),
                leg,
                onto,
                |cell, (at, _)| free(cell, at),
                |place| {
                    paths::passable(heights.neighbours(place), end, |cell, (at, _)| {
                        crowd.stops(cell, at, entity, false)
                    })
                },
                |(at, _)| at,
            );
            (leg, course)
        };
        let follow = |planned: &mut Option<(Vec3, paths::Course<_>)>| {
            planned.as_mut().and_then(|(leg, course)| {
                paths::follow(
                    course,
                    here,
                    (level, false),
                    *leg,
                    |cell, (at, _)| free(cell, at),
                    |(at, _)| at,
                )
            })
        };
        let at_leg = |planned: &Option<(Vec3, paths::Course<_>)>| {
            planned.as_ref().is_some_and(|(leg, _)| {
                (*leg - here).with_y(0.0).length() <= AT_LEG && (*leg - target).length() > 0.0
            })
        };
        if walk.course.is_none() || at_leg(&walk.course) {
            walk.course = match plan(here) {
                (leg, Some(course)) => Some((leg, course)),
                (_, None) => None,
            };
        }
        let mut step = follow(&mut walk.course);
        if step.is_none() && walk.course.is_some() {
            walk.course = match plan(here) {
                (leg, Some(course)) => Some((leg, course)),
                (_, None) => None,
            };
            step = follow(&mut walk.course);
        }
        let (from, toward) = match step {
            Some(paths::Step::Through((next, _))) => {
                walk.level = next;
                continue;
            }
            Some(paths::Step::Toward(from, point)) => (from, point),
            // Its way's next step no longer clear: it stands, as at a creature in its way.
            Some(paths::Step::Blocked) => (here.xz(), here),
            None => (here.xz(), target),
        };
        let to = (toward - here).with_y(0.0);
        let distance = to.length();
        let left = (target - here).with_y(0.0).length();
        let mut position =
            paths::advance(from, toward, here, walk.speed * speed.0 * time.delta_secs());
        let into = paths::cell_of(position.x, position.z);
        let blocked =
            into != cell && !free(into, walk.level) && heights.walkable_on(cell, walk.level);
        // Within its target's reach, on the level it walks onto unless its way found none.
        let there = left <= walk.within
            && (onto.is_none_or(|onto| onto == walk.level) || walk.course.is_none());
        if there || blocked || distance <= f32::EPSILON {
            // There, or as near as it gets: it stands; held from its first step, a while.
            if walk.moved || there {
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
        (Without<Strikes>, Without<Dies>, Without<ScriptClip>),
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

/// How a scripts' clip blends in and back to standing (`0x544B6A`, `0x544F06`, `0x545204`).
const CLIP_IN: Duration = Duration::from_millis(250);
const DEFENDING_BACK: f32 = 0.2;

/// Plays each creature's scripts' clip ([`ScriptClip`]): its slot's clip of its model, added to
/// its graph the first time, blended in over 0.25 s at its attack speed's hundredth (`0x584FE0`;
/// 1.0 for a creature of none), looped its count of times; it ends at the clip's end, or 0.2 s
/// before it for the slots 53 to 65 (`0x545204`), and the creature stands again (`stance`). A
/// slot its model lacks plays nothing.
fn script_clips(
    mut commands: Commands,
    data: Res<Npcs>,
    mut npcs: Query<(Entity, &mut Npc, &mut ScriptClip, Option<&fight::Combat>), Without<Dies>>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    (gltfs, clips): (Res<Assets<Gltf>>, Res<Assets<AnimationClip>>),
    mut graphs: ResMut<Assets<AnimationGraph>>,
    speed: Res<hero::GameSpeed>,
) {
    for (entity, mut npc, mut clip, combat) in &mut npcs {
        let Some(player_entity) = npc.player else {
            continue;
        };
        let Ok((mut player, mut transitions)) = players.get_mut(player_entity) else {
            continue;
        };
        if let Some((node, handle)) = &clip.playing {
            let length = clips.get(handle).map_or(0.0, AnimationClip::duration);
            let ended = player.animation(*node).is_none_or(|active| {
                active.is_finished()
                    || (DEFENDING.contains(&clip.slot)
                        && active.completions() + 1 >= clip.loops
                        && active.seek_time() >= length - DEFENDING_BACK)
            });
            if ended {
                commands.entity(entity).remove::<ScriptClip>();
            }
            continue;
        }
        let handle = data
            .slots
            .get(&npc.model)
            .and_then(|slots| slots.slot(clip.slot))
            .and_then(|name| {
                gltfs
                    .get(&npc.gltf)
                    .and_then(|gltf| gltf.named_animations.get(name).cloned())
            });
        let Some(graph) = npc.graph.as_mut() else {
            continue;
        };
        let scripted = match graph.scripted.get(&clip.slot) {
            Some(scripted) => Some(scripted.clone()),
            None => {
                let added = handle.and_then(|handle| {
                    let mut graph_asset = graphs.get_mut(&graph.graph)?;
                    let root = graph_asset.root;
                    Some((graph_asset.add_clip(handle.clone(), 1.0, root), handle))
                });
                if let Some(added) = &added {
                    graph.scripted.insert(clip.slot, added.clone());
                }
                added
            }
        };
        let Some((node, handle)) = scripted else {
            bevy::log::debug!(
                "npcs: {entity} has no clip of the slot {} to play",
                clip.slot
            );
            commands.entity(entity).remove::<ScriptClip>();
            continue;
        };
        let rate = combat.map_or(1.0, |combat| combat.0.speed / 100.0);
        let active = transitions.play(&mut player, node, CLIP_IN);
        active.replay();
        active
            .set_repeat(bevy::animation::RepeatAnimation::Count(clip.loops))
            .set_speed(speed.0 * rate);
        clip.playing = Some((node, handle));
    }
}

/// A door, which the townsfolk go into the houses and out of them by (`ai`): its type, by which
/// it sounds (`sfx::door_profile`), its model, its clips of opening and closing (`*_ACTIVATE`,
/// `*_DEACTIVATE`) and its animation player once it has them, and whether it is open (bit 0 of
/// its trigger's word, `0x4FC8D0`).
#[derive(Component)]
pub struct Door {
    kind: u32,
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

    /// Places the records of the world that the scripts ran during the play (#1014): the places
    /// of their `DefPos`s, and the creatures and objects of their `CreateNPC`s and `CreateObj`s,
    /// kept as an `Init`'s and taking their levels as they are made, at once where their sectors
    /// are in the world ([`place`]), else as they come in, each `CreateNPC` of the type that its
    /// sector's area's `RegionChange` list gave it as it ran (`area` the area of a sector, #1063);
    /// their `RegionChange`s on the creatures kept away from the world that they reach, in order
    /// among them (#1064, [`Self::change_kept`]); their `SetNPCState`s on the creatures they make.
    /// How many they placed, and the `RegionChange`s for the creatures in the world
    /// ([`region_changes`]).
    pub fn add(
        &mut self,
        records: &[Record],
        area: impl Fn((u32, u32)) -> Option<u8>,
    ) -> Result<Added> {
        let records: Vec<&Record> = records.iter().collect();
        self.places.extend(positions(records.iter().copied()));
        let targets = goto_targets(&records);
        let side = u32::try_from(SIDE)?;
        let mut made: HashMap<*const Record, usize> = HashMap::new();
        let mut changes = Vec::new();
        for &record in &records {
            let Body::Operands(ops) = &record.body else {
                continue;
            };
            if record.kind == REGION_CHANGE
                && let Some(change) = quests::AreaChange::of(ops)
            {
                self.change_kept(&change, &area)?;
                changes.push(change);
                continue;
            }
            if record.kind != CREATE_NPC && record.kind != CREATE_OBJ {
                continue;
            }
            let Some(placement) = self.makers.placement(
                record,
                ops,
                Origin::default(),
                (&self.places, &targets),
                &mut self.slots,
            )?
            else {
                continue;
            };
            let (x, y) = placement.cell;
            let at = (x / side, y / side);
            let placement = match area(at).filter(|_| placement.turns()) {
                Some(area) => match area_turn(ops, area) {
                    Some(turn) => match self.makers.retype(&placement, turn, &mut self.slots)? {
                        Some(turned) => turned,
                        None => continue,
                    },
                    None => placement,
                },
                None => placement,
            };
            let index = self.placements.len();
            if record.kind == CREATE_OBJ
                && let (Some(name), Some(trigger)) = (name_of(ops), placement.trigger.clone())
            {
                self.named.insert(name, trigger);
            }
            self.by_sector.entry(at).or_default().push(index);
            self.placements.push(placement);
            self.played.insert(index);
            made.insert(std::ptr::from_ref(record), index);
            if self.spawned.contains(&at) {
                self.pending.push(index);
            }
        }
        set_states(&records, &made, &mut self.placements);
        Ok(Added {
            placed: made.len(),
            changes,
        })
    }

    /// Changes the creatures that the `RegionChange` `change` reaches away from the world
    /// (`0x4ABFE0`, #1064), each of its area where it stands (`area` the area of a sector): the
    /// start's, the `Init`s' and the play's placements that are not in the world, where they
    /// went or were made, their health kept as its share ([`health_kept`]), and the fills' living
    /// creatures that went with their sectors. An `Enter`'s, made again at each coming, takes the
    /// lists then (#1063).
    fn change_kept(
        &mut self,
        change: &quests::AreaChange,
        area: &impl Fn((u32, u32)) -> Option<u8>,
    ) -> Result<()> {
        let side = u32::try_from(SIDE)?;
        let reaches = |(x, y): (u32, u32)| area((x / side, y / side)) == Some(change.area);
        for index in 0..self.placements.len() {
            let placement = &self.placements[index];
            if placement.made.0 || !placement.changes() || self.kept.out.contains(&index) {
                continue;
            }
            let went = self.kept.went.get(&index).copied();
            let cell = match went {
                Some(Some(stood)) => stood.cell,
                Some(None) => continue,
                None => placement.cell,
            };
            if !reaches(cell) {
                continue;
            }
            let Some(changed) = self.makers.change(placement, change, &mut self.slots)? else {
                continue;
            };
            if let Some(Some(stood)) = self.kept.went.get_mut(&index) {
                stood.health = health_kept(stood.health, &changed, &self.fight);
            }
            self.placements[index] = changed;
        }
        for gone in self.kept.filled.values_mut() {
            for (placement, stood) in gone.iter_mut() {
                if !placement.changes() || !reaches(stood.cell) {
                    continue;
                }
                if let Some(changed) = self.makers.change(placement, change, &mut self.slots)? {
                    stood.health = health_kept(stood.health, &changed, &self.fight);
                    *placement = changed;
                }
            }
        }
        Ok(())
    }

    /// The creatures that the play's `NPC_Goto`s send walking and its `Teleport`s put somewhere
    /// ([`mover`]), the hero's teleports outside the anim mode left to `scripts` (#1035): their
    /// placements away from the world moved to a cell they go to, the moves returned in order for
    /// the creatures in the world and the hero ([`script_records`]), those between a
    /// `SetAnimMode` or a `SetCinemaMode` and a `SetPlayMode` queued (#1220; a `SetPlayMode`
    /// naming a creature, which ends the mode as that creature's queue reaches it, ends it at
    /// once); a `SetCinemaMode` stops the hero first (#1280).
    fn moves(&mut self, records: &[Record], now: f64) -> Vec<Moved> {
        let mut moves = Vec::new();
        let mut queued = false;
        for record in records {
            match record.kind {
                SET_ANIM_MODE => queued = true,
                SET_CINEMA_MODE => {
                    queued = true;
                    moves.push(Moved {
                        name: HERO.to_owned(),
                        act: Act::Halt,
                        queued: false,
                    });
                }
                SET_PLAY_MODE => queued = false,
                _ => {}
            }
            let Body::Operands(ops) = &record.body else {
                continue;
            };
            if let Some(moved) = played(record.kind, ops, queued, now) {
                moves.push(moved);
                continue;
            }
            let Some(name) = mover(record.kind, ops, queued) else {
                continue;
            };
            let walks = record.kind == NPC_GOTO;
            let Some(whither) = [0x04, 0x0C, 0x0D]
                .into_iter()
                .find_map(|tag| whither(ops, tag, &self.places))
            else {
                continue;
            };
            if let Whither::Cell((x, y, layer)) = whither
                && name != HERO
                && let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y))
            {
                for (index, placement) in self.placements.iter_mut().enumerate() {
                    if placement.name.as_ref() == Some(&name) && !self.kept.out.contains(&index) {
                        placement.cell = (x, y);
                        placement.layer = layer;
                    }
                }
            }
            moves.push(Moved {
                name,
                act: Act::Go { whither, walks },
                queued,
            });
        }
        moves
    }

    /// The dialogue that the start's `SetRgnDialog`s give the creature type `creature`.
    pub fn region_talk(&self, creature: u32) -> Option<&str> {
        self.region_talks.get(&creature).map(String::as_str)
    }

    /// The icon of the dialogue `talk`'s entry, the first of its name case aside (`+0x48`,
    /// #1039), none for a dialogue the table does not hold.
    pub fn icon(&self, talk: &str) -> Option<i32> {
        let index = self
            .dialogues
            .iter()
            .position(|name| name.eq_ignore_ascii_case(talk))?;
        self.icons.get(index).copied()
    }

    /// What the play's `SetNPCState`s give the creatures made before them (#656, #658, #657):
    /// each record's name and its state, given to the placements of that name here, and to the
    /// creatures in the world by [`script_records`] ([`State::restates`]).
    fn restate(&mut self, records: &[Record]) -> Vec<(String, State)> {
        let made: HashSet<String> = records
            .iter()
            .filter(|record| record.kind == CREATE_NPC)
            .filter_map(|record| match &record.body {
                Body::Operands(ops) => name_of(ops),
                _ => None,
            })
            .collect();
        let mut stated = Vec::new();
        for record in records.iter().filter(|record| record.kind == SET_NPC_STATE) {
            let Body::Operands(ops) = &record.body else {
                continue;
            };
            let Some(name) = name_of(ops).filter(|name| !made.contains(name)) else {
                continue;
            };
            let state = State::of(ops, &self.dialogues);
            if !state.restates() {
                continue;
            }
            for placement in &mut self.placements {
                if placement.name.as_ref() == Some(&name) {
                    state.restate_placement(placement);
                }
            }
            stated.push((name, state));
        }
        stated
    }

    /// What the play's `SetGroupState`s and `DelNPC`s do (#1021): each given to the placements
    /// of its group or name here, a removal keeping them from being placed again, and returned,
    /// in order, for the creatures in the world ([`script_records`]).
    fn regroup(&mut self, records: &[Record]) -> Vec<Regrouped> {
        let mut regrouped = Vec::new();
        for record in records {
            let Body::Operands(ops) = &record.body else {
                continue;
            };
            let change = match record.kind {
                SET_GROUP_STATE => {
                    let state = GroupState::of(ops, &self.dialogues);
                    if state.group.is_none() {
                        continue;
                    }
                    Regrouped::Group(state)
                }
                DEL_NPC => match name_of(ops) {
                    Some(name) => Regrouped::Removed(name),
                    None => continue,
                },
                _ => continue,
            };
            for index in 0..self.placements.len() {
                let placement = &self.placements[index];
                let member = match &change {
                    Regrouped::Group(state) => placement.group == state.group,
                    Regrouped::Removed(name) => placement.name.as_ref() == Some(name),
                };
                if !member || placement.object {
                    continue;
                }
                match &change {
                    Regrouped::Group(state) if !state.removes => {
                        state.apply(&mut self.placements[index]);
                    }
                    // Removed where it is not in the world, its `Enter`'s made again as theirs
                    // come in (`sdk_DESTROY`).
                    _ if !placement.made.0 && !self.kept.out.contains(&index) => {
                        self.kept.went.insert(index, None);
                    }
                    _ => {}
                }
            }
            regrouped.push(change);
        }
        regrouped
    }

    /// The scripts' groups that hold a creature living away from the world, wherever it stands,
    /// as `GroupIsDead` asks of each member (`0x48C860`): a placement not in the world now, not
    /// an `Enter`'s, made by the start or the play and never killed, or gone with its sector
    /// alive. A sector's `Init` makes its creatures only as it comes near; the world's creatures
    /// are the caller's.
    pub fn living_groups(&self) -> HashSet<u32> {
        self.placements
            .iter()
            .enumerate()
            .filter_map(|(index, placement)| {
                let group = placement.group?;
                if placement.object || placement.made.0 || self.kept.out.contains(&index) {
                    return None;
                }
                let lives = match self.kept.went.get(&index) {
                    Some(stood) => stood.is_some(),
                    None => placement.started || self.played.contains(&index),
                };
                lives.then_some(group)
            })
            .collect()
    }

    /// Whether `door`'s trigger is locked (#1006): it opens to nobody (`0x44BCD0`).
    pub fn locked(&self, door: &Door) -> bool {
        door.trigger
            .as_ref()
            .is_some_and(|name| self.locked.contains(name))
    }
}

/// A door that `ai` or the hero (`doors`) opens or closes, as the event `0x21` turns it
/// (`0x4FC8D0`, `0x44BCD0`): the event of a creature that is no hero leaves open a door that a
/// hero turned before.
#[derive(Component)]
pub struct Swing {
    pub open: bool,
    by_hero: bool,
    /// The scripts' setting of its trigger's bit (`SetObjState`, `0x415F00`, `0x416280`), which
    /// turns it whatever its bar and its mark (#952).
    scripted: bool,
}

impl Swing {
    /// A townsperson's opening (`true`) or closing of a door.
    pub fn to(open: bool) -> Self {
        Self {
            open,
            by_hero: false,
            scripted: false,
        }
    }

    /// The hero's.
    pub fn by_hero(open: bool) -> Self {
        Self {
            open,
            by_hero: true,
            scripted: false,
        }
    }

    /// The scripts'.
    pub fn by_script(open: bool) -> Self {
        Self {
            open,
            by_hero: false,
            scripted: true,
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

/// Opens or closes each door that `ai` swings, playing its clip once, and its sound of opening
/// or closing before it, at the door, at a rate's factor of 0.45 and a tenth's roll (`0x44BCD0`,
/// `0x695800`; #1166).
fn swing(
    mut commands: Commands,
    mut doors: Query<(Entity, &mut Door, &Swing)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    mut npcs: ResMut<Npcs>,
    mut heights: ResMut<Heights>,
    speed: Res<hero::GameSpeed>,
    mut sounds: MessageWriter<sfx::Play>,
) {
    let mut sound = |entity, door: &Door, open: bool| {
        if door.open == open {
            return;
        }
        let Some(profile) = sfx::door_profile(door.kind) else {
            return;
        };
        let group = if open { sfx::OPEN } else { sfx::CLOSE };
        sounds.write(sfx::Play {
            sound: sfx::Of::Profile(profile, group),
            at: Some(entity),
            rate: (0.45, sfx::Roll::Tenths),
        });
    };
    for (entity, mut door, swing) in &mut doors {
        if !swing.open && !swing.by_hero && !swing.scripted && npcs.marked(&door) {
            commands.entity(entity).remove::<Swing>();
            continue;
        }
        // A locked door opens to nobody, and closes as an unlocked one does (`0x44BCD0`).
        if swing.open && !swing.scripted && !door.open && npcs.locked(&door) {
            bevy::log::debug!("npcs: {entity} is locked");
            commands.entity(entity).remove::<Swing>();
            continue;
        }
        let (Some((open, close)), Some(player)) = (door.clips, door.player) else {
            // Its clips not yet given, it waits, unless its model has none.
            if door.player.is_some() {
                sound(entity, &door, swing.open);
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
        sound(entity, &door, swing.open);
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
    open_trigger(npcs, heights, name, swing.open);
}

/// Opens or closes the cells of the trigger `name`, bit 0 of its state, and keeps the state for
/// the sectors that come in.
fn open_trigger(npcs: &mut Npcs, heights: &mut Heights, name: &str, open: bool) {
    let spots = npcs.triggers.get(name).cloned().unwrap_or_default();
    if open {
        npcs.opened.insert(name.to_owned());
    } else {
        npcs.opened.remove(name);
    }
    for spot in spots {
        for (cell, level) in spot_levels(heights, spot) {
            if open {
                heights.free(cell, level);
            } else {
                heights.occupy(cell, level);
            }
        }
    }
}

/// An NPC's name in the scripts, by which their records name it (`CreateNPC`'s `0x01`,
/// lower-cased).
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct ScriptName(pub String);

/// An NPC's dialogue (`+0x245`, #1028): its name in the dialogues' table, whose function
/// `Dialog:<name>` runs as the hero speaks to it (`talk`).
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct Talk(pub String);

/// An NPC's trades (`+0x200` bits, `opensacred_rules::quests::SMITH` and the others), as its
/// `CreateNPC`'s and the start's `SetNPCState`s' tags give them ([`TRADES`]), by which it shows
/// its icon (#1039).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trades(pub u32);

/// An NPC's death's functions (#656, docs/re/netscript.md, "The deaths' functions"), of which its
/// death runs one (`scripts`).
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct OnDeath(pub DeathFunctions);

/// The placement that an `Enter`'s creature was made of at its sector's coming, which a
/// `RegionChange` changes it from (#1064, [`region_changes`]).
#[derive(Component, Clone)]
struct Remakes(Box<Placement>);

/// A creature that a `RegionChange` made again (#1064): it goes, its new self in its place, and
/// what it leaves is not kept ([`keep`]).
#[derive(Component)]
struct Remade;

/// The play's `RegionChange`s, in order, that the creatures in the world take
/// ([`region_changes`]).
#[derive(Resource, Default)]
pub struct AreaChanges(Vec<quests::AreaChange>);

/// Carries out the play's `RegionChange`s on the creatures in the world (`0x4ABFE0`, #1064;
/// [`Npcs::add`] those away from it): each creature of a placement that its area's change turns,
/// where it stands now, is made again of the new type in its place, on its floor, with its share
/// of health, its group, its dialogue and its death's functions, its sector's still; a kept one's
/// placement changes with it. The hero's followers and the creatures of no placement (the
/// console's, the summons) are not changed.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn region_changes(
    mut commands: Commands,
    mut changes: ResMut<AreaChanges>,
    mut npcs: ResMut<Npcs>,
    mut streamer: ResMut<Streamer>,
    (heights, states, assets): (Res<Heights>, Res<buildings::Buildings>, Res<AssetServer>),
    creatures: Query<
        (
            Entity,
            &Transform,
            &crowd::InTheWay,
            &fight::Health,
            &ai::Mind,
            (
                Option<&Placed>,
                Option<&Remakes>,
                Option<&OnDeath>,
                Option<&Talk>,
            ),
        ),
        (Without<Dies>, Without<crate::followers::Hired>),
    >,
) {
    if changes.0.is_empty() {
        return;
    }
    let changes = std::mem::take(&mut changes.0);
    let npcs = &mut *npcs;
    let side = u32::try_from(SIDE).unwrap_or(64);
    for (entity, at, way, health, mind, (placed, remakes, on_death, talk)) in &creatures {
        let placement = match (placed.map(|placed| &placed.source), remakes) {
            (Some(Source::Kept(index)), _) => npcs.placements.get(*index).cloned(),
            (Some(Source::Filled(placement)), _) => Some((**placement).clone()),
            (None, Some(remakes)) => Some((*remakes.0).clone()),
            (None, None) => None,
        };
        let Some(mut placement) = placement else {
            continue;
        };
        let (x, y) = paths::cell_of(at.translation.x, at.translation.z);
        let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
            continue;
        };
        let sector = (x / side, y / side);
        let area = streamer.area(sector);
        let mut changed = false;
        for change in changes.iter().filter(|change| Some(change.area) == area) {
            if !placement.changes() {
                break;
            }
            match npcs.makers.change(&placement, change, &mut npcs.slots) {
                Ok(Some(new)) => {
                    placement = new;
                    changed = true;
                }
                Ok(None) => {}
                Err(e) => bevy::log::warn!("npcs: {e:#}"),
            }
        }
        if !changed {
            continue;
        }
        bevy::log::info!(
            "npcs: {entity}, a {}, is changed into a {} of side {} by its area's RegionChange",
            mind.creature,
            placement.creature.0,
            placement.side
        );
        if let Some(Source::Kept(index)) = placed.map(|placed| &placed.source)
            && let Some(kept) = npcs.placements.get_mut(*index)
        {
            *kept = placement.clone();
        }
        // Made again where it stands, with what the scripts gave it since.
        let mut there = placement.clone();
        there.cell = (x, y);
        there.layer = heights
            .floor(way.level)
            .map_or(0, |(_, floor)| i32::from(floor));
        there.walk = None;
        there.health_share = None;
        there.group = mind.group;
        there.on_death = on_death
            .map(|on_death| on_death.0.clone())
            .unwrap_or_default();
        there.talk = talk.map(|talk| talk.0.clone());
        let kept = health_kept(*health, &there, &npcs.fight);
        let made = spawn(
            &mut commands,
            &there,
            &heights,
            &states,
            &assets,
            (health.level, &npcs.fight, &npcs.items),
        );
        commands.entity(made).insert(kept);
        match placed {
            Some(placed) => {
                let source = match &placed.source {
                    Source::Kept(index) => Source::Kept(*index),
                    Source::Filled(_) => Source::Filled(Box::new(placement)),
                };
                commands.entity(made).insert(Placed {
                    at: placed.at,
                    source,
                });
            }
            None => {
                commands.entity(made).insert(Remakes(Box::new(placement)));
            }
        }
        commands.entity(entity).insert(Remade).despawn();
        streamer.disown(entity);
        streamer.adopt(sector, made);
    }
}

/// Carries out the scripts' `SetHP`s (`0x48E280`, #1001): the creatures of the name, the hero for
/// none, take the share of their most health, truncated; a creature of none dies, as by no
/// player's hand (it drops nothing).
pub(crate) fn script_healths(
    mut commands: Commands,
    healths: Option<ResMut<crate::scripts::Healths>>,
    mut creatures: Query<(Entity, &ScriptName, &mut fight::Health), Without<Dies>>,
    mut heroes: Query<&mut fight::Health, (With<hero::Body>, Without<ScriptName>)>,
) {
    let Some(mut healths) = healths else {
        return;
    };
    for set in std::mem::take(&mut healths.0) {
        let Some(name) = &set.name else {
            for mut health in &mut heroes {
                health.current = health.most * set.share / 100;
            }
            bevy::log::info!("npcs: the scripts set the hero's health to {} %", set.share);
            continue;
        };
        for (entity, named, mut health) in &mut creatures {
            if &named.0 != name {
                continue;
            }
            health.current = health.most * set.share / 100;
            bevy::log::info!(
                "npcs: the scripts set {name}'s health to {}/{}",
                health.current,
                health.most
            );
            if health.current == 0 {
                commands.entity(entity).insert(Dies::unkilled());
            }
        }
    }
}

/// The steps that the play's records give a creature under the scripts' anim mode (#1220;
/// docs/re/netscript.md, "The anim mode and the creatures' queues"): its queue (`+0x588`), whose
/// first step it takes at each tick of the world ([`take_steps`], `0x59A840`), its own idle
/// behaviours left meanwhile (`ai`).
#[derive(Component, Default, Debug)]
pub struct ScriptQueue(std::collections::VecDeque<Queued>);

/// A step of a creature's [`ScriptQueue`].
#[derive(Clone, Debug, PartialEq)]
enum Queued {
    /// A run to `to` (`NPC_Goto`, the kind 1) onto the level `onto`, the creature's own as it
    /// sets out where the record names no floor (**hypothesis** for when: the original takes the
    /// creature's layer as the record runs, so that its walk after a teleport down never counts
    /// as arrived and is dropped standing at its target); how many times it was ordered, and where
    /// the creature stood as the count began.
    Walk {
        to: Vec3,
        onto: Option<Level>,
        orders: u8,
        noted: Vec3,
    },
    /// A teleport to `to` on the level `level` (`Teleport`, the kind 9).
    Put { to: Vec3, level: Level },
    /// The hero's teleport to `to`, landing there as `arrival` says, as the scripts' `Teleport`
    /// puts a player (the kind 9 of the local hero's queue, #1280).
    Land { to: Vec3, arrival: hero::Arrival },
    /// A clip of the slot `slot` (`PlayAnim`, the kind 3): waits while a one-off clip of the
    /// creature plays, then starts and is done (#1277).
    Clip { slot: u16 },
    /// Another record's turn, of its kind, done at once (#1277; their own steps #1279 to #1281).
    Other(u16),
    /// A `CallFunktion`'s function, run at its turn and done the same tick (the step `0x59CE5B`,
    /// #1278).
    Call(String),
    /// A `Wait` (the kind 2): holds the queue while its condition `until` holds of the other
    /// creature, `None` the hero (#1279).
    Wait { other: Option<String>, until: Until },
}

/// A `Wait`'s condition (the step `0x59CAA5`, the table `0x59F538`), which holds the queue while
/// it holds.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Until {
    /// `0x58`: while the anim mode is on, which no span of the Sacred campaign keeps past its run.
    Mode,
    /// `0x59`: while the other fights.
    Fights,
    /// `0x5A`: while it lives.
    Lives,
    /// `0x5B`: while it stands idle.
    Idle,
    /// `0x5C`: until it stands idle, while it is busy.
    Busy,
    /// `0x5D`, `0x5E`: a variable's bit (not followed: done at once).
    Bit,
    /// `0x5F`: a number of seconds of the real clock, of hundredths where negative, counted from
    /// the record's whole second or hundredth; the deadline in seconds once the record ran.
    Time(f64),
}

impl Until {
    /// The condition of a `Wait`'s tag `tag` and its number `n`, the real clock at `now`
    /// seconds.
    fn of(tag: u8, n: i32, now: f64) -> Option<Self> {
        Some(match tag {
            0x58 => Self::Mode,
            0x59 => Self::Fights,
            0x5A => Self::Lives,
            0x5B => Self::Idle,
            0x5C => Self::Busy,
            0x5D | 0x5E => Self::Bit,
            0x5F if n >= 0 => Self::Time(now.floor() + f64::from(n)),
            0x5F => Self::Time(((now * 100.0).floor() + f64::from(n.unsigned_abs())) / 100.0),
            _ => return None,
        })
    }

    /// Whether the queue waits on: the other's state `other`, whether it fights and whether it
    /// stands idle, none where it is gone; the real clock at `now`.
    fn holds(self, other: Option<(bool, bool)>, now: f64) -> bool {
        match (self, other) {
            (Self::Mode | Self::Bit, _)
            | (Self::Fights | Self::Lives | Self::Idle | Self::Busy, None) => false,
            (Self::Fights, Some((fights, _))) => fights,
            (Self::Lives, Some(_)) => true,
            (Self::Idle, Some((_, idle))) => idle,
            (Self::Busy, Some((_, idle))) => !idle,
            (Self::Time(deadline), _) => now < deadline,
        }
    }

    /// Whether the waiting creature is held and stopped meanwhile (`+0x200` bit `0x10`,
    /// `0x54E6D0`): for the conditions on the mode and on another creature.
    fn stops(self) -> bool {
        !matches!(self, Self::Bit | Self::Time(_))
    }
}

/// A clip that the scripts play on a creature (`PlayAnim`; the standing action's sub-states
/// `0x65` and `0x66`, `0x544B6A`): its slot, how many times it plays, and its node once it plays.
/// The creature's idle behaviours leave it be meanwhile (`ai`), and its stance waits (`stance`).
#[derive(Component, Debug)]
pub struct ScriptClip {
    slot: u16,
    loops: u32,
    playing: Option<(AnimationNodeIndex, Handle<AnimationClip>)>,
}

impl ScriptClip {
    fn new(slot: u16, loops: u32) -> Self {
        Self {
            slot,
            loops,
            playing: None,
        }
    }
}

/// How near its target, along each axis, a creature stands for its queue's walk to end
/// (`0x59A885`): less than a cell.
const QUEUED_ARRIVES: f32 = 40.0;
/// How many times a queue's walk is ordered before it is dropped where its creature has not
/// moved (`0x59A885`), and how far it counts as having moved.
const QUEUED_ORDERS: u8 = 5;
const QUEUED_MOVED: f32 = 1.0;

/// Takes the first step of each creature's [`ScriptQueue`] at each tick of the world
/// (`0x5375C0` calling `0x59A840`): a walk runs to its target, waiting while the creature fights
/// or moves, and ends once it stands within 40 of the target along each axis on the walk's
/// level, or once it was ordered more than five times without the creature moving; a teleport
/// puts the creature there at once. The queue goes once empty.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn take_steps(
    mut commands: Commands,
    tick: Res<WorldTick>,
    (heights, real): (Res<Heights>, Res<Time<bevy::time::Real>>),
    mut due: Option<ResMut<crate::scripts::Due>>,
    others: Query<
        (
            &ScriptName,
            Option<&ai::Mind>,
            Has<Walk>,
            Has<ScriptClip>,
            Has<Plays>,
        ),
        Without<Dies>,
    >,
    heroes: Query<&hero::Body>,
    mut creatures: Query<
        (
            Entity,
            &mut ScriptQueue,
            &mut Transform,
            Option<&ai::Mind>,
            Option<&mut crowd::InTheWay>,
            Option<&mut buildings::Layered>,
            Has<Walk>,
            (Has<ScriptClip>, Has<Plays>),
        ),
        (Without<Dies>, Without<hero::Body>),
    >,
) {
    if tick.seconds.is_none() {
        return;
    }
    for (entity, mut queue, mut transform, mind, way, layered, walking, (clip, plays)) in
        &mut creatures
    {
        let here = transform.translation;
        let level = way.as_ref().and_then(|way| way.level);
        let done = match queue.0.front_mut() {
            None => true,
            Some(Queued::Walk {
                to,
                onto,
                orders,
                noted,
            }) => {
                if walking || mind.is_some_and(|mind| mind.fighting()) {
                    false
                } else {
                    let onto = *onto.get_or_insert(level);
                    let arrived = (to.x - here.x).abs() < QUEUED_ARRIVES
                        && (to.z - here.z).abs() < QUEUED_ARRIVES
                        && onto == level;
                    let dropped =
                        *orders > QUEUED_ORDERS && (here - *noted).xz().length() < QUEUED_MOVED;
                    if *orders > QUEUED_ORDERS {
                        *orders = 0;
                    }
                    if !arrived && !dropped {
                        if *orders == 0 {
                            *noted = here;
                        }
                        *orders += 1;
                        // At a run (`+0x1F8` 2, `0x59A8D9`).
                        let speed = mind.map_or(0.0, |mind| mind.run);
                        commands
                            .entity(entity)
                            .insert(Walk::run(*to, level, speed).onto(onto).idle());
                    }
                    arrived || dropped
                }
            }
            Some(&mut Queued::Put { to, level }) => {
                let mut at = to;
                if let Some(y) = heights.standing(at.x, at.z, level) {
                    at.y = y;
                }
                transform.translation = at;
                if let Some(mut way) = way {
                    way.level = level;
                }
                if let Some(mut layered) = layered {
                    *layered = buildings::Layered::at(&heights, at.x, at.z, level);
                }
                commands.entity(entity).remove::<Walk>();
                true
            }
            // A one-off clip ordered or playing, the scripts' or an instrument's, makes it wait
            // (`0x59E151`); a walk does not, its walk overwritten.
            Some(&mut Queued::Clip { slot }) => {
                if clip || plays {
                    false
                } else {
                    commands
                        .entity(entity)
                        .remove::<Walk>()
                        .insert(ScriptClip::new(slot, 1));
                    true
                }
            }
            Some(Queued::Other(_) | Queued::Land { .. }) => true,
            Some(Queued::Call(function)) => {
                if let Some(due) = due.as_deref_mut() {
                    due.0.push(std::mem::take(function));
                }
                true
            }
            // The other's state: whether it fights and whether it stands idle (the action 0 and
            // the AI state 0, a script's clip or an instrument's waited for too), none where it
            // is gone; the hero idle where it goes nowhere.
            Some(Queued::Wait { other, until }) => {
                let state = match other {
                    None => heroes
                        .iter()
                        .next()
                        .map(|body| (false, body.target().is_none())),
                    Some(name) => others.iter().find(|(named, ..)| named.0 == *name).map(
                        |(_, mind, walks, clip, plays)| {
                            let fights = mind.is_some_and(|mind| mind.fighting());
                            (fights, !fights && !walks && !clip && !plays)
                        },
                    ),
                };
                let holds = until.holds(state, real.elapsed_secs_f64());
                if holds && until.stops() {
                    commands.entity(entity).remove::<Walk>();
                }
                !holds
            }
        };
        if done {
            queue.0.pop_front();
        }
        if queue.0.is_empty() {
            commands.entity(entity).remove::<ScriptQueue>();
        }
    }
}

/// Takes the first step of the hero's [`ScriptQueue`] at each tick of the world, as the hero's
/// update steps its queue as a creature's (`0x5375C0` calling `0x59A840` for the local hero, seen
/// in `annehmen2003`'s run, #1280): a walk orders the hero to its target at a run, the player's
/// gait set to it (`+0x1F8` 2, seen kept after), waits while it goes, and ends once it stands
/// within 40 of the target along each axis on the walk's level, or once it was ordered more than
/// five times without the hero moving; a teleport puts it there as the scripts' `Teleport` puts a
/// player. The Sacred campaign queues no other step for the hero. The queue goes once empty.
fn take_hero_steps(
    mut commands: Commands,
    tick: Res<WorldTick>,
    hero: Option<ResMut<hero::Hero>>,
    mut heroes: Query<(Entity, &mut ScriptQueue, &mut hero::Body, &mut Transform)>,
) {
    if tick.seconds.is_none() {
        return;
    }
    let Ok((entity, mut queue, mut body, mut transform)) = heroes.single_mut() else {
        return;
    };
    let here = transform.translation;
    let level = body.level();
    let done = match queue.0.front_mut() {
        None => true,
        Some(Queued::Walk {
            to,
            onto,
            orders,
            noted,
        }) => {
            if body.target().is_some() || body.ordered() {
                false
            } else {
                let onto = *onto.get_or_insert(level);
                let arrived = (to.x - here.x).abs() < QUEUED_ARRIVES
                    && (to.z - here.z).abs() < QUEUED_ARRIVES
                    && onto == level;
                let dropped =
                    *orders > QUEUED_ORDERS && (here - *noted).xz().length() < QUEUED_MOVED;
                if *orders > QUEUED_ORDERS {
                    *orders = 0;
                }
                if !arrived && !dropped {
                    if *orders == 0 {
                        *noted = here;
                    }
                    *orders += 1;
                    if let Some(mut hero) = hero {
                        hero.gait = hero::Gait::Run;
                    }
                    body.send_onto(*to, onto);
                }
                arrived || dropped
            }
        }
        Some(&mut Queued::Land { to, arrival }) => {
            body.put(&mut transform, to, arrival);
            true
        }
        Some(_) => true,
    };
    if done {
        queue.0.pop_front();
    }
    if queue.0.is_empty() {
        commands.entity(entity).remove::<ScriptQueue>();
    }
}

/// Places the records of the world that the scripts ran during the play (#1014,
/// [`Npcs::add`]).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
pub(crate) fn script_records(
    mut commands: Commands,
    records: Option<ResMut<crate::scripts::WorldRecords>>,
    mut npcs: ResMut<Npcs>,
    (heights, heroes, mut bodies, streamer, mut changes): (
        Res<Heights>,
        Query<&Transform, With<hero::Body>>,
        Query<(Entity, &mut hero::Body)>,
        Res<Streamer>,
        ResMut<AreaChanges>,
    ),
    mut creatures: Query<
        (
            Entity,
            Option<&ScriptName>,
            Option<&mut OnDeath>,
            &mut Transform,
            Option<&mut ai::Mind>,
            Option<&mut crowd::InTheWay>,
            Option<&mut fight::Health>,
            Option<&Talk>,
            Option<&mut buildings::Layered>,
        ),
        (Without<Dies>, Without<hero::Body>),
    >,
    (mut queues, real): (Query<&mut ScriptQueue>, Res<Time<bevy::time::Real>>),
    dying: Query<(&ScriptName, &Transform, Option<&crowd::InTheWay>), With<Dies>>,
) {
    // A creature's dialogue as a change leaves it, its mind's flag with it.
    let talk_to = |commands: &mut Commands, entity: Entity, talk: Option<String>| match talk {
        Some(talk) => {
            commands.entity(entity).insert(Talk(talk));
        }
        None => {
            commands.entity(entity).remove::<Talk>();
        }
    };
    let Some(mut records) = records else {
        return;
    };
    let mut records = std::mem::take(&mut records.0);
    if records.is_empty() {
        return;
    }
    // The places of the creatures that the records name (`CPOS:`), the hero's for none, as they
    // stand now: their cells and floors (#1046).
    let floor_of = |level| {
        heights
            .floor(level)
            .map_or(0, |(_, floor)| i32::from(floor))
    };
    let cell_at = |at: Vec3, floor: i32| {
        let (x, y) = crate::paths::cell_of(at.x, at.z);
        (x, y, floor)
    };
    locate(&mut records, |name| match name {
        None => heroes.iter().next().map(|at| {
            let floor = bodies
                .iter()
                .next()
                .map_or(0, |(_, body)| floor_of(body.level()));
            cell_at(at.translation, floor)
        }),
        // A dying creature still stands where it fell, as its death's function runs while its
        // object lives on (#1295): quest 204's guard lays its book there.
        Some(name) => creatures
            .iter()
            .find(|(_, named, ..)| named.is_some_and(|named| named.0 == name))
            .map(|(_, _, _, at, _, way, ..)| {
                cell_at(at.translation, way.map_or(0, |way| floor_of(way.level)))
            })
            .or_else(|| {
                dying
                    .iter()
                    .find(|(named, ..)| named.0 == name)
                    .map(|(_, at, way)| {
                        cell_at(at.translation, way.map_or(0, |way| floor_of(way.level)))
                    })
            }),
    });
    match npcs.add(&records, |at| streamer.area(at)) {
        Ok(added) => {
            if added.placed > 0 {
                bevy::log::info!(
                    "npcs: the scripts place {} creatures and objects",
                    added.placed
                );
            }
            changes.0.extend(added.changes);
        }
        Err(e) => bevy::log::warn!("npcs: the scripts' records: {e:#}"),
    }
    let icons = set_icons(&records);
    let npcs = &mut *npcs;
    reicon(&npcs.dialogues, &mut npcs.icons, icons);
    for (name, state) in npcs.restate(&records) {
        for (entity, _, on_death, mut transform, mut mind, _, _, talk, _) in creatures
            .iter_mut()
            .filter(|(_, named, ..)| named.is_some_and(|named| named.0 == name))
        {
            if let Some(hire) = state.hire {
                commands
                    .entity(entity)
                    .insert(crate::followers::Hired(hire));
            }
            if state.talk.is_some() || state.stocked {
                let talk = state.talk_of(talk.map(|talk| talk.0.clone()));
                if let Some(mind) = mind.as_mut() {
                    mind.dialogue = talk.is_some();
                }
                talk_to(&mut commands, entity, talk);
            }
            match on_death {
                Some(mut on_death) => state.restate(&mut on_death.0),
                None if state.deaths.is_empty() => {}
                None => {
                    let mut functions = DeathFunctions::default();
                    state.restate(&mut functions);
                    commands.entity(entity).insert(OnDeath(functions));
                }
            }
            if let Some(degrees) = state.heading {
                transform.rotation = hero::facing(heading(degrees));
            }
            if let Some(mut mind) = mind {
                if state.group.is_some() {
                    mind.group = state.group;
                }
                if state.stocked {
                    mind.dialogue = false;
                }
            }
            bevy::log::info!("npcs: the scripts set {name}'s state");
        }
    }
    for change in npcs.regroup(&records) {
        for (entity, named, on_death, _, mind, way, health, ..) in &mut creatures {
            let member = match &change {
                Regrouped::Group(state) => {
                    mind.as_ref().is_some_and(|mind| mind.group == state.group)
                }
                Regrouped::Removed(name) => named.is_some_and(|named| &named.0 == name),
            };
            if !member {
                continue;
            }
            match (&change, mind) {
                (Regrouped::Group(state), Some(mut mind)) if !state.removes => {
                    let mut functions = on_death.as_ref().map(|d| d.0.clone()).unwrap_or_default();
                    state.apply_to(&mut mind, way.map(|way| way.into_inner()), &mut functions);
                    talk_to(&mut commands, entity, state.talk.clone());
                    match on_death {
                        Some(mut on_death) => on_death.0 = functions,
                        None if functions.is_empty() => {}
                        None => {
                            commands.entity(entity).insert(OnDeath(functions));
                        }
                    }
                }
                // Removed without a death, its function not run, nothing dropped: kept from
                // coming back as it goes (`sdk_DESTROY(id, 1, 0, 1)`, `0x463A70`).
                _ => {
                    if let Some(mut health) = health {
                        health.current = 0;
                    }
                    commands.entity(entity).despawn();
                }
            }
        }
        match change {
            Regrouped::Group(state) => bevy::log::info!(
                "npcs: the scripts {} the group {}",
                if state.removes { "remove" } else { "set" },
                state.group.map_or(0, |group| group & !SCRIPT_GROUP)
            ),
            Regrouped::Removed(name) => bevy::log::info!("npcs: the scripts remove {name}"),
        }
    }
    // The steps queued in this run, for the creatures without a queue yet.
    let mut queued: HashMap<Entity, Vec<Queued>> = HashMap::new();
    for moved in npcs.moves(&records, real.elapsed_secs_f64()) {
        let (whither, walks) = match moved.act {
            Act::Go { whither, walks } => (whither, walks),
            // The hero stopped where its queue is empty (#1280).
            Act::Halt => {
                if let Ok((entity, mut body)) = bodies.single_mut()
                    && queues.get(entity).is_err()
                    && !queued.contains_key(&entity)
                {
                    body.stop();
                    bevy::log::info!("npcs: the scripts' cinema mode stops the hero");
                }
                continue;
            }
            Act::Clip { slot, loops } => {
                for (entity, ..) in creatures
                    .iter()
                    .filter(|(_, named, ..)| named.is_some_and(|named| named.0 == moved.name))
                {
                    if moved.queued {
                        let step = Queued::Clip { slot };
                        match queues.get_mut(entity) {
                            Ok(mut queue) => queue.0.push_back(step),
                            Err(_) => queued.entry(entity).or_default().push(step),
                        }
                    } else {
                        // Outside the mode, the event `0x108` (**hypothesis**: the same clip, its
                        // count its loops).
                        commands
                            .entity(entity)
                            .remove::<Walk>()
                            .insert(ScriptClip::new(slot, loops));
                    }
                }
                bevy::log::info!(
                    "npcs: the scripts play the slot {slot} of {}{}",
                    moved.name,
                    if moved.queued { " in its turn" } else { "" }
                );
                continue;
            }
            act @ (Act::Other(_) | Act::Call(_) | Act::Wait(..)) => {
                for (entity, ..) in creatures
                    .iter()
                    .filter(|(_, named, ..)| named.is_some_and(|named| named.0 == moved.name))
                {
                    let step = match &act {
                        Act::Call(function) => Queued::Call(function.clone()),
                        Act::Other(kind) => Queued::Other(*kind),
                        Act::Wait(other, until) => Queued::Wait {
                            other: other.clone(),
                            until: *until,
                        },
                        _ => continue,
                    };
                    match queues.get_mut(entity) {
                        Ok(mut queue) => queue.0.push_back(step),
                        Err(_) => queued.entry(entity).or_default().push(step),
                    }
                }
                continue;
            }
        };
        // A creature's place: the hero's, or the first creature's of the name.
        let spot = match &whither {
            Whither::Cell((x, y, layer)) => {
                let floor = usize::try_from(*layer).ok().filter(|&f| f > 0);
                let level = floor.and_then(|floor| {
                    let building = heights.building_on_floor((*x, *y), floor)?;
                    heights.floor_level(building, floor)
                });
                Some((iso::ground(*x as f32 + 0.5, *y as f32 + 0.5), level, *layer))
            }
            Whither::Creature(None) => heroes.iter().next().map(|at| (at.translation, None, 1)),
            Whither::Creature(Some(name)) => creatures
                .iter()
                .find(|(_, named, ..)| named.is_some_and(|named| &named.0 == name))
                .map(|(_, _, _, at, _, way, ..)| {
                    (at.translation, way.and_then(|way| way.level), 1)
                }),
        };
        let Some((to, level, layer)) = spot else {
            continue;
        };
        // The hero's own (#1280): under the mode a step of its queue, which it takes as a
        // creature does (seen: `annehmen2003`'s run, then its teleport); outside it, an
        // `NPC_Goto`'s walk at once, its teleport the scripts' own.
        if moved.name == HERO {
            let Ok((entity, mut body)) = bodies.single_mut() else {
                continue;
            };
            if moved.queued {
                let step = if walks {
                    Queued::Walk {
                        to,
                        onto: (layer > 0).then_some(level),
                        orders: 0,
                        noted: heroes.iter().next().map_or(to, |at| at.translation),
                    }
                } else {
                    let arrival = match &whither {
                        Whither::Cell(_) => hero::Arrival::On(u8::try_from(layer).ok()),
                        Whither::Creature(_) => hero::Arrival::Nearest,
                    };
                    Queued::Land { to, arrival }
                };
                match queues.get_mut(entity) {
                    Ok(mut queue) => queue.0.push_back(step),
                    Err(_) => queued.entry(entity).or_default().push(step),
                }
            } else if walks {
                match (layer > 0).then_some(level) {
                    Some(level) => body.send_onto(to, level),
                    None => body.send(to),
                }
            }
            bevy::log::info!(
                "npcs: the scripts {} the hero to {:?}{}",
                if walks { "send" } else { "put" },
                whither,
                if moved.queued { " in its turn" } else { "" }
            );
            continue;
        }
        for (entity, _, _, mut transform, mind, way, _, _, layered) in creatures
            .iter_mut()
            .filter(|(_, named, ..)| named.is_some_and(|named| named.0 == moved.name))
        {
            // Under the anim mode, a step of its queue, in its turn (#1220): a walk to a cell
            // of no floor onto the creature's own level (`0x49E210`), a teleport's floor 0 the
            // ground.
            if moved.queued {
                let step = if walks {
                    Queued::Walk {
                        to,
                        onto: (layer > 0).then_some(level),
                        orders: 0,
                        noted: transform.translation,
                    }
                } else {
                    Queued::Put { to, level }
                };
                match queues.get_mut(entity) {
                    Ok(mut queue) => queue.0.push_back(step),
                    Err(_) => queued.entry(entity).or_default().push(step),
                }
                continue;
            }
            if walks {
                // At its walk speed, as the start's `NPC_Goto`s walk theirs (#132).
                let speed = mind.as_ref().map_or(0.0, |mind| mind.speed);
                let from = way.as_ref().and_then(|way| way.level);
                commands
                    .entity(entity)
                    .insert(Walk::to(to, from, speed).onto(level).idle());
            } else {
                let mut at = to;
                if let Some(y) = heights.standing(at.x, at.z, level) {
                    at.y = y;
                }
                transform.translation = at;
                if let Some(mut way) = way {
                    way.level = level;
                }
                if let Some(mut layered) = layered {
                    *layered = buildings::Layered::at(&heights, at.x, at.z, level);
                }
                commands.entity(entity).remove::<Walk>();
            }
        }
        bevy::log::info!(
            "npcs: the scripts {} {} to {:?}{}",
            if walks { "send" } else { "put" },
            moved.name,
            whither,
            if moved.queued { " in its turn" } else { "" }
        );
    }
    for (entity, steps) in queued {
        commands.entity(entity).insert(ScriptQueue(steps.into()));
    }
}

/// Carries out the scripts' `SetObjState`s (`0x48AE90`, #952): each names an object, whose
/// trigger it bars or unbars and opens or closes, its doors in the world turning with it. The
/// state goes to the trigger by the object's name whether the object is in the world or gone
/// with its sector, and the next object made under that name takes it
/// (docs/decisions/netscript.md, "Object states kept for a gone object's name").
fn script_states(
    mut commands: Commands,
    states: Option<ResMut<crate::scripts::ObjStates>>,
    mut npcs: ResMut<Npcs>,
    mut heights: ResMut<Heights>,
    doors: Query<(Entity, &Door)>,
) {
    let Some(mut states) = states else {
        return;
    };
    for state in std::mem::take(&mut states.0) {
        let Some(trigger) = npcs.named.get(&state.name).cloned() else {
            bevy::log::debug!(
                "npcs: SetObjState of {}, which names no object with a trigger",
                state.name
            );
            continue;
        };
        bevy::log::info!(
            "npcs: the scripts set {}'s trigger {trigger}: barred {:?}, open {:?}",
            state.name,
            state.barred,
            state.open
        );
        match state.barred {
            Some(true) => {
                npcs.locked.insert(trigger.clone());
            }
            Some(false) => {
                npcs.locked.remove(&trigger);
            }
            None => {}
        }
        let Some(open) = state.open else {
            continue;
        };
        let mut turned = false;
        for (entity, door) in &doors {
            if door.trigger.as_deref() == Some(trigger.as_str()) {
                commands.entity(entity).insert(Swing::by_script(open));
                turned = true;
            }
        }
        if !turned {
            open_trigger(&mut npcs, &mut heights, &trigger, open);
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
    /// Whether a creature killed it, not the hero: it drops nothing (`0x533BA8`, #630).
    pub by_creature: bool,
    /// Whether anyone killed it: a `SetHP` of 0, a summoned follower replaced or a lifetime's
    /// end kills nobody's, which the scripts' kills do not count (`0x534F60`, #1021).
    pub killed: bool,
    /// Whether it is an undead that rises again (`undead`, #1199): its death drops nothing,
    /// gives no experience and is no kill (`0x533C7F`, `0x534775`).
    pub rises: bool,
}

impl Dies {
    pub fn new(choice: u32) -> Self {
        Self {
            choice,
            played: false,
            by_creature: false,
            killed: true,
            rises: false,
        }
    }

    /// A death at nobody's hands, which drops nothing (#1021).
    pub fn unkilled() -> Self {
        Self {
            by_creature: true,
            killed: false,
            ..Self::new(0)
        }
    }

    /// A death at a creature's hands, with its dying clip `choice`.
    pub fn by_creature(choice: u32) -> Self {
        Self {
            by_creature: true,
            ..Self::new(choice)
        }
    }
}

/// What a dead NPC was, which the Vampiress's awakening (1034) raises: its type, its
/// `Creature.pak` class, whether it had a dialogue, and the level it lies on.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Corpse {
    pub creature: u32,
    pub class: u8,
    pub dialogue: bool,
    pub level: Level,
}

/// Lays each dying NPC down: its dying clip once, its walk, its strike, its mind and its place in
/// the way gone, what it was kept ([`Corpse`]).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
pub(crate) fn die(
    mut commands: Commands,
    mut dying: Query<(
        Entity,
        &Npc,
        &mut Dies,
        Option<&ai::Mind>,
        Option<&crowd::InTheWay>,
    )>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    speed: Res<hero::GameSpeed>,
) {
    for (entity, npc, mut dies, mind, way) in &mut dying {
        if dies.played {
            continue;
        }
        if let (Some(mind), Some(way)) = (mind, way) {
            bevy::log::debug!("npcs: {entity}, a {}, lies dead", mind.creature);
            commands.entity(entity).insert(Corpse {
                creature: mind.creature,
                class: mind.class,
                dialogue: mind.dialogue,
                level: way.level,
            });
        }
        commands
            .entity(entity)
            .remove::<(Walk, Strikes, Talks, Plays, crowd::InTheWay, ai::Mind)>();
        // A creature of the class 5 at its last death plays no dying clip: it burns away,
        // bursts or vanishes (`0x53F31D`, #1199, #1260).
        if mind.is_some_and(|mind| mind.class == opensacred_rules::undead::CLASS) && !dies.rises {
            dies.played = true;
            continue;
        }
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

/// How far a strike reaches when it hits, times the reach of the striker and its target, the mean
/// of their sizes (`0x4EA1F0`, `0x4DBC60`, #677; **hypothesis** that the base is the reach of the
/// fight's band).
const HITS_WITHIN: f32 = 3.0;

/// How far a strike of a creature of the size `size` reaches a target of the size `other` when
/// it hits ([`HITS_WITHIN`] times their reach, `opensacred_rules::minds::reach`).
fn hits_within(size: u32, other: u32) -> f32 {
    HITS_WITHIN * f32::from(u16::try_from(minds::reach(size, other)).unwrap_or(u16::MAX))
}

/// Plays each striking NPC's attack once, and at the first tick of the world past the clip's
/// fractions (0.5 for a first of 0; a second when it is above 0), or at its end, sends a hit of
/// its share when the target is then within reach, as the action steps at the ticks alone (#617);
/// a walk ends the strike (`0x4E4CF0`, `0x4EA1F0`). A shot plays its clip at its attack speed
/// times its chain, and at the first tick past the clip's first fraction, or at its end, looses
/// its arrow (`arrows::Loose`, `0x4DD90E`).
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn strike(
    mut commands: Commands,
    tick: Res<WorldTick>,
    mut strikers: Query<
        (
            Entity,
            &Npc,
            &mut Strikes,
            Has<Walk>,
            &GlobalTransform,
            Option<&fight::Combat>,
            Option<&crate::ai::Mind>,
        ),
        (Without<Dies>, Without<crate::making::Knocked>),
    >,
    places: Query<(
        &GlobalTransform,
        Option<&hero::Size>,
        Option<&crate::ai::Mind>,
    )>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    clips: Res<Assets<AnimationClip>>,
    speed: Res<hero::GameSpeed>,
    mut hits: MessageWriter<fight::Hit>,
    mut shots: MessageWriter<arrows::Loose>,
    mut blows: MessageWriter<crate::sfx::Blow>,
) {
    for (entity, npc, mut strikes, walking, at, combat, mind) in &mut strikers {
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
        // At a tick, or as the clip ends between two, which the next tick's thought then sees.
        let due = (tick.seconds.is_some() || active.is_finished())
            && match (strikes.shot, strikes.struck) {
                (Some(_), 0) => opensacred_rules::minds::looses(fractions[0], done),
                (None, 0) => done >= first,
                (None, 1) => fractions[1] > 0.0 && done >= fractions[1],
                _ => false,
            };
        if due && strikes.shot.is_some() {
            strikes.struck += 1;
            blows.write(crate::sfx::Blow::Shot { shooter: entity });
            shots.write(arrows::Loose {
                shooter: entity,
                target: strikes.target,
                special: None,
            });
        } else if due {
            strikes.struck += 1;
            let size = mind.map_or(minds::SIZE, |mind| mind.size());
            let reaches = places
                .get(strikes.target)
                .is_ok_and(|(target, hero, other)| {
                    let other = hero
                        .map(|hero| hero.0)
                        .or_else(|| other.map(|mind| mind.size()))
                        .unwrap_or(minds::SIZE);
                    (target.translation() - at.translation())
                        .with_y(0.0)
                        .length()
                        <= hits_within(size, other)
                });
            if reaches {
                // The swing within reach (`0x4EA6FD`, #712).
                blows.write(crate::sfx::Blow::Swing { striker: entity });
                hits.write(fight::Hit {
                    fixed: None,
                    sure: false,
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

/// Levels each boss, champion or creature of a champion's levelling still unfought
/// (`fight::Unfought`) at its first strike or hit, as the hit is sent and before it is resolved
/// (`0x56B130` for both sides, `0x550E50`, #630): its opponent's level, a creature's or the
/// hero's, plus `LevelUpBoss` or `LevelUpChamp`, held within the levels of the sector it stands
/// in (`opensacred_rules::fight::Rank::level`); its health refilled to its new most, its items
/// made again at its new level, and its attack, defence, damage, resistances, regeneration and
/// pace made anew.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn first_fight(
    mut commands: Commands,
    mut hits: MessageReader<fight::Hit>,
    npcs: Option<Res<Npcs>>,
    experience: Res<fight::Experience>,
    heroes: Query<(), With<hero::Body>>,
    levels: Query<&fight::Health, Without<fight::Unfought>>,
    mut unfought: Query<(
        &fight::Unfought,
        &fight::Ranked,
        &Transform,
        &mut ai::Mind,
        &mut fight::Health,
        &mut fight::Combat,
        &mut Arms,
        &mut Npc,
        Option<&crate::effects::Effects>,
    )>,
) {
    let Some(npcs) = npcs else {
        return;
    };
    let mut levelled = HashSet::new();
    for hit in hits.read() {
        for (who, opponent) in [(hit.attacker, hit.target), (hit.target, hit.attacker)] {
            if levelled.contains(&who) || !unfought.contains(who) {
                continue;
            }
            let opponent = if heroes.contains(opponent) {
                experience.level
            } else if let Ok(health) = levels.get(opponent) {
                health.level
            } else if let Ok((.., health, _, _, _, _)) = unfought.get(opponent) {
                health.level
            } else {
                continue;
            };
            let Ok((
                unfought,
                rank,
                transform,
                mut mind,
                mut health,
                mut combat,
                mut arms,
                mut npc,
                effects,
            )) = unfought.get_mut(who)
            else {
                continue;
            };
            let here = transform.translation;
            let (x, y) = paths::cell_of(here.x, here.z);
            let side = i32::try_from(SIDE).unwrap_or(64);
            let at = (
                u32::try_from(x.div_euclid(side)).unwrap_or(0),
                u32::try_from(y.div_euclid(side)).unwrap_or(0),
            );
            let values = npcs.values.get(&at).copied().unwrap_or_default();
            let rules = &npcs.fight;
            let grade = rank.0.level(1, opponent, &values, unfought.big, rules);
            let creature = mind.creature;
            let mut dice = spawning::Rng::of(
                u32::try_from(x).unwrap_or(0),
                u32::try_from(y).unwrap_or(0),
                usize::try_from(creature).unwrap_or(0),
            );
            for (held, values) in &mut arms.holding {
                *values = npcs.items.make(held.item, grade, &mut dice);
            }
            let regenerates = relevel(
                grade,
                rank.0,
                (&mut *mind, &mut *health, &mut *combat, &mut *npc),
                &arms,
                rules,
                &effects
                    .map(crate::effects::Effects::list)
                    .unwrap_or_default(),
            );
            // The level's setter starts its experience anew at its level (#690).
            commands
                .entity(who)
                .remove::<(fight::Unfought, fight::Points)>()
                .insert(regenerates);
            levelled.insert(who);
            bevy::log::info!(
                "npcs: {who}, a {creature} of rank {:?}, levelled at its first fight to {grade} by \
                 its opponent's {opponent}, health {}",
                rank.0,
                health.most
            );
        }
    }
}

/// Gives each creature that killed another the victim's experience (the event `0x10D`,
/// `0x5311F5`, #690) through `addExperience` (`0x57E160`): `0x57D860`'s experience of the victim
/// at the killer's level, twice or 1.5 times for a boss or a champion, added to the killer's
/// points, which start at its level's start (`fight::Points`). Once they pass its level's end,
/// the level's setter (`0x564D60`, with `a` 1 and `b` 0) gives it their level held at the sector
/// it stands in (`opensacred_rules::fight::Rank::held`) and makes its values anew with its health
/// refilled, its items as they are, even where the hold keeps its level, as the original's does
/// at each such kill. The owner's share (`+0x1EC`) is not followed; a blue potion's effect adds
/// to the kill's experience (#1353).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn earn(
    mut commands: Commands,
    mut kills: MessageReader<fight::Kill>,
    npcs: Option<Res<Npcs>>,
    mut killers: Query<(
        Option<&mut fight::Points>,
        Option<&fight::Ranked>,
        &Transform,
        &mut ai::Mind,
        &mut fight::Health,
        &mut fight::Combat,
        &Arms,
        &mut Npc,
        Option<&crate::effects::Effects>,
    )>,
) {
    let Some(npcs) = npcs else {
        return;
    };
    let rules = &npcs.fight;
    for kill in kills.read() {
        let Ok((
            points,
            ranked,
            transform,
            mut mind,
            mut health,
            mut combat,
            arms,
            mut npc,
            effects,
        )) = killers.get_mut(kill.killer)
        else {
            continue;
        };
        let from = health.level;
        // A blue potion's effect adds to it (#1353).
        let bonus = effects.map_or(0.0, |effects| {
            opensacred_rules::potions::mentored(&effects.list())
        });
        let earned = kill.rank.earned(fight::experience_with(
            kill.worth.0,
            kill.level,
            from,
            rules,
            bonus,
        ));
        let mut sum = points
            .as_deref()
            .map_or_else(|| fight::level_start(from), |points| points.0);
        let reached = fight::creature_earns(&mut sum, from, u64::from(earned));
        match points {
            Some(mut points) => points.0 = sum,
            None => {
                commands.entity(kill.killer).insert(fight::Points(sum));
            }
        }
        bevy::log::info!(
            "npcs: {}, a {} of level {from}, earns {earned} experience by its kill, {sum} in all",
            kill.killer,
            mind.creature
        );
        let Some(reached) = reached else {
            continue;
        };
        let rank = ranked.map_or(fight::Rank::Common, |ranked| ranked.0);
        let values = npcs.values.get(&sector_of(transform.translation));
        let grade = rank.held(reached, values, mind.creature, mind.big, rules);
        let regenerates = relevel(
            grade,
            rank,
            (&mut *mind, &mut *health, &mut *combat, &mut *npc),
            arms,
            rules,
            &effects
                .map(crate::effects::Effects::list)
                .unwrap_or_default(),
        );
        commands.entity(kill.killer).insert(regenerates);
        bevy::log::info!(
            "npcs: {}, a {} of level {from}, reaches the level {reached} by its kills' \
             experience, held at {grade}, health {}",
            kill.killer,
            mind.creature,
            health.most
        );
    }
}

/// The sector that the point `here` of the world stands in.
fn sector_of(here: Vec3) -> (u32, u32) {
    let (x, y) = paths::cell_of(here.x, here.z);
    let side = i32::try_from(SIDE).unwrap_or(64);
    (
        u32::try_from(x.div_euclid(side)).unwrap_or(0),
        u32::try_from(y.div_euclid(side)).unwrap_or(0),
    )
}

/// Makes a creature's values anew at its level `grade` and of its rank `rank`, as the level's
/// setter does (`0x564D60` and its recomputation `0x5796A0`): its skills, its most health with its
/// health refilled to it, its attack, defence, damage, resistances and pace with its items as
/// they are and its effects `effects`, its hits' share, and its regeneration, which it returns.
fn relevel(
    grade: u32,
    rank: fight::Rank,
    (mind, health, combat, npc): (
        &mut ai::Mind,
        &mut fight::Health,
        &mut fight::Combat,
        &mut Npc,
    ),
    arms: &Arms,
    rules: &fight::Rules,
    effects: &[Effect],
) -> fight::Regenerates {
    let skills = fight::skills(&arms.base, grade, arms.plain);
    let factor = rank.health(mind.creature, mind.class, rules);
    let most = if arms.plain {
        PLAIN_HEALTH
    } else {
        fight::most_health(arms.base.attributes, grade, &skills, factor, rules)
    };
    *health = fight::Health {
        current: most,
        most,
        level: grade,
    };
    mind.level = grade;
    fight::Regenerates::new(refit(
        arms,
        grade,
        rank,
        rules,
        effects,
        (mind, combat, npc),
    ))
}

/// Gives an NPC the pace `pace`, its walk and run speeds by it.
pub(crate) fn repace(mind: &mut ai::Mind, npc: &mut Npc, pace: f32) {
    let ratio = pace / npc.pace.max(f32::EPSILON);
    mind.speed *= ratio;
    mind.run *= ratio;
    npc.pace = pace;
}

/// Makes an NPC's attack, defence, damage, resistances and pace anew at its level `grade` and of
/// its rank `rank` with its effects `effects` (#763), its walk and run by its new pace, as
/// [`rearm`] makes them with its weapons, and its hits' share; returns its regeneration's value
/// with them.
pub(crate) fn refit(
    arms: &Arms,
    grade: u32,
    rank: fight::Rank,
    rules: &fight::Rules,
    effects: &[Effect],
    (mind, combat, npc): (&mut ai::Mind, &mut fight::Combat, &mut Npc),
) -> f32 {
    let (values, share) = arms.fight_with(grade, rank, rules, effects);
    // Its pace times its time's factor, the effect 15's (`0x537160`).
    let pace = values.0.pace_factor() * opensacred_rules::fight::time_factor(effects);
    repace(mind, npc, pace);
    npc.share = share;
    *combat = values;
    let skills = fight::skills(&arms.base, grade, arms.plain);
    opensacred_rules::fight::regeneration_with(&arms.base, grade, &skills, effects, rules)
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
    /// What it holds, for its weapons' sounds ([`Npcs::armed`]); bare hands without the NPCs'
    /// tables.
    pub fn armed(&self, npcs: Option<&Npcs>) -> crate::sfx::Armed {
        let in_hands: Vec<InHand> = self.holding.iter().map(|&(held, _)| held).collect();
        npcs.map_or_else(crate::sfx::Armed::default, |npcs| npcs.armed(&in_hands))
    }

    /// Its attack, defence, damage and resistances at level `grade` and of rank `rank`
    /// (`0x5796A0`), with its skills and its items' values: unarmed when what it holds is of
    /// class 0 and its right hand empty (`0x546F80`); and its hits' share of a strike, halved with
    /// two weapons, the class 8 or one in each hand, the left no shield (`0x4EA3F0`).
    fn fight(&self, grade: u32, rank: fight::Rank, rules: &fight::Rules) -> (fight::Combat, f32) {
        self.fight_with(grade, rank, rules, &[])
    }

    /// [`Self::fight`] with its effects `effects` (#763).
    fn fight_with(
        &self,
        grade: u32,
        rank: fight::Rank,
        rules: &fight::Rules,
        effects: &[Effect],
    ) -> (fight::Combat, f32) {
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
            gun: holds_kind(&in_hands, &GUNS),
            items: self
                .holding
                .iter()
                .filter_map(|&(held, values)| {
                    values.map(|values| fight::ItemValues {
                        class: held.category,
                        ..values
                    })
                })
                .collect(),
            skill: skill_of(class, &self.holding),
        };
        let skills = fight::skills(&self.base, grade, self.plain);
        let mut values =
            opensacred_rules::fight::combat_with(&self.base, grade, held, &skills, effects, rules);
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
            equipment
                .0
                .retain(|item| !item.mount.handed() || !gone(item.hand));
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
                    mount: Mount::held(gear.category),
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
        repace(&mut mind, &mut npc, values.0.pace_factor());
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
            mount: Mount::Hand,
            ready: true,
            waits: 0,
        }]));
    }
}

/// An NPC that plays its talk once where it stands (`*_TALK_A`, slot 177; `ai`'s meetings).
#[derive(Component)]
pub struct Talks;

/// Notes the quests' items that the hero took, which their sectors place no more (#1256).
fn taken_items(
    (mut taken, mut items): (
        MessageReader<crate::pickup::TakenForQuest>,
        MessageReader<crate::pickup::TakenPlaced>,
    ),
    mut npcs: ResMut<Npcs>,
) {
    for taken in taken.read() {
        npcs.taken.insert(taken.index);
    }
    for taken in items.read() {
        npcs.taken.insert(taken.0);
    }
}

/// The NPCs and objects that the scripts and the fills place: placed as their sectors come in,
/// animated, walked, retextured and equipped.
pub struct NpcsPlugin;

impl Plugin for NpcsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Retextured>()
            .init_resource::<AreaChanges>()
            .add_observer(retexture)
            .add_observer(equipment_ready)
            .add_observer(keep)
            .add_systems(
                Update,
                (
                    script_records,
                    taken_items,
                    region_changes,
                    script_states,
                    script_healths,
                    place,
                    rearm,
                    animate,
                    door_clips,
                    (take_steps, take_hero_steps).chain(),
                    walk,
                    put,
                    swing,
                    instrument,
                    strike,
                    die,
                    script_clips,
                    stance,
                    equip,
                    bind_worn,
                )
                    .chain()
                    .in_set(Step::Npcs),
            )
            .add_systems(Update, first_fight.before(fight::resolve))
            .add_systems(Update, earn.after(fight::resolve));
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;
    use std::fs;

    use bevy::prelude::Vec2;
    use opensacred_data::netscript::{Function, Operand};

    use super::*;

    #[test]
    fn a_createnpc_of_the_tag_0x11_joins_its_group_apart_from_the_fills_packs() {
        let op = |tag, value| Operand { tag, value };
        // The start's bears by the boss 689, of one group (#669).
        let bear = [
            op(0x08, Value::Marker),
            op(GROUP, Value::I32(815)),
            op(0x46, Value::Marker),
        ];
        assert_eq!(script_group(&bear), Some(SCRIPT_GROUP | 815));
        assert_eq!(script_group(&[op(GROUP, Value::I32(0))]), None);
        assert_eq!(script_group(&[op(0x08, Value::Marker)]), None);
        // The last counts; a pack's number, below 10,000,000, never meets a script's.
        let twice = [op(GROUP, Value::I32(3)), op(GROUP, Value::I32(4))];
        assert_eq!(script_group(&twice), Some(SCRIPT_GROUP | 4));
        const { assert!(SCRIPT_GROUP > 99 * 100_000 + 99 * 1000 + 999) };
    }

    #[test]
    fn textures_are_found_by_their_record_number() {
        let assets =
            std::env::temp_dir().join(format!("opensacred-game-textures-{}", std::process::id()));
        let dir = assets.join("textures").join("texture");
        fs::create_dir_all(&dir).unwrap();
        for name in [
            "07442-chest02.ktx2",
            "00001-cab.ktx2",
            "notes.txt",
            "x-y.ktx2",
        ] {
            fs::write(dir.join(name), b"").unwrap();
        }
        let files = texture_files(&assets).unwrap();
        fs::remove_dir_all(&assets).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[&7442], "textures/texture/07442-chest02.ktx2");
        assert_eq!(files[&1], "textures/texture/00001-cab.ktx2");
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
    fn a_socket_is_placed_by_the_scales_above_it_and_posed_without_them() {
        let close = |a: Vec3, b: Vec3| (a - b).length() < 1e-5;
        // A dagger's socket under a node of scale 0.4 (#814): its translation counts 0.4 of
        // itself, turned by the node; the pose keeps the rotations and no scale.
        let blade = Transform::from_xyz(0.0, -2.0, 0.0)
            .with_rotation(Quat::from_rotation_z(FRAC_PI_2))
            .with_scale(Vec3::splat(0.4));
        let socket = Transform::from_xyz(7.0, 0.5, 1.0)
            .with_rotation(Quat::from_rotation_x(-FRAC_PI_2))
            .with_scale(Vec3::splat(0.25));
        let pose = Chain::ROOT.then(&blade).then(&socket).pose();
        assert!(close(pose.translation, Vec3::new(-0.2, 0.8, 0.4)));
        let turned = blade.rotation * socket.rotation;
        assert!(pose.rotation.abs_diff_eq(turned, 1e-6));
        assert_eq!(pose.scale, Vec3::ONE);
        // A bone of a general scale/shear `R · U · diag(s) · Uᵀ`, which the converter writes as
        // an outer node turned by `R · U` and scaled, and an inner one turned by `Uᵀ`: the pose
        // turns by `R`, the bone's own rotation, and its child lies where the matrix puts it.
        let r = Quat::from_rotation_y(0.7);
        let u = Quat::from_rotation_x(0.3);
        let outer = Transform::from_xyz(1.0, 2.0, 3.0)
            .with_rotation(r * u)
            .with_scale(Vec3::new(0.5, 2.0, 1.0));
        let inner = Transform::from_rotation(u.inverse());
        let child = Transform::from_xyz(0.0, 1.0, 0.0);
        let sheared = Chain::ROOT.then(&outer).then(&inner);
        assert!(sheared.pose().rotation.abs_diff_eq(r, 1e-6));
        let matrix = outer.to_matrix() * inner.to_matrix();
        let expected = matrix.transform_point3(Vec3::Y);
        assert!(close(sheared.then(&child).pose().translation, expected));
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
    fn a_goto_or_a_teleport_sends_its_creature_to_a_cell_a_place_or_a_creature() {
        let op = |tag, value| Operand { tag, value };
        let named = HashMap::from([("novizin1".to_owned(), (3237, 2514))]);
        assert_eq!(
            whither(&[op(0x04, Value::I32x3([3237, 2519, 1]))], 0x04, &named),
            Some(Whither::Cell((3237, 2519, 1)))
        );
        assert_eq!(
            whither(&[op(0x04, Value::Name("Novizin1".into()))], 0x04, &named),
            Some(Whither::Cell((3237, 2514, 0)))
        );
        assert_eq!(
            whither(&[op(0x04, Value::Str("CPOS:HERO".into()))], 0x04, &named),
            Some(Whither::Creature(None))
        );
        assert_eq!(
            whither(
                &[op(0x04, Value::Str("CPOS:res:17190".into()))],
                0x04,
                &named
            ),
            Some(Whither::Creature(Some("res:17190".into())))
        );
        assert_eq!(
            whither(&[op(0x04, Value::Name("nowhere".into()))], 0x04, &named),
            None
        );
        assert_eq!(
            whither(&[op(0x0C, Value::I32x3([1, 2, 0]))], 0x04, &named),
            None
        );
    }

    #[test]
    fn a_set_group_state_changes_its_members_or_removes_them() {
        let op = |tag, value| Operand { tag, value };
        let dialogues = [
            "first".to_owned(),
            "first".to_owned(),
            "Priester".to_owned(),
        ];
        // Quest 1101's priests made hostile, awake, with a death's function and no dialogue.
        let state = GroupState::of(
            &[
                op(ON_DEATH, Value::Str("Priesterweg1101".into())),
                op(0x08, Value::Marker),
                op(GROUP, Value::I32(11012)),
                op(0x12, Value::Marker),
            ],
            &dialogues,
        );
        assert_eq!(state.group, Some(SCRIPT_GROUP | 11012));
        assert!(!state.removes && !state.dialogue);
        let mut priest = Placement {
            dialogue: true,
            side: 3,
            creature: (677, 0, 0x8_0400),
            ..Placement::default()
        };
        state.apply(&mut priest);
        assert_eq!(priest.creature.2, 0x8_0001);
        assert_eq!(priest.side, 2);
        assert!(!priest.dialogue);
        assert_eq!(
            priest.on_death.clone().take().as_deref(),
            Some("Priesterweg1101")
        );
        // A dialogue found gives one; a removal is only that.
        let talks = GroupState::of(
            &[
                op(GROUP, Value::I32(5)),
                op(DIALOGUE, Value::Str("priester".into())),
            ],
            &dialogues,
        );
        assert!(talks.dialogue);
        let removes = GroupState::of(
            &[op(GROUP, Value::I32(10)), op(REMOVES, Value::Marker)],
            &[],
        );
        assert!(removes.removes);
    }

    #[test]
    fn a_set_npc_state_turns_the_creature_groups_it_and_stocks_a_horse() {
        let op = |tag, value| Operand { tag, value };
        let mut placement = Placement {
            dialogue: true,
            heading: 10.0,
            ..Placement::default()
        };
        let state = State::of(
            &[
                op(0x01, Value::Str("Res:18104".into())),
                op(0x03, Value::U16(45)),
                op(GROUP, Value::I32(2404)),
                op(STOCK, Value::Str("res:17466".into())),
            ],
            &[],
        );
        assert!(state.restates());
        state.apply(&mut placement);
        assert_eq!(placement.heading, 45.0);
        assert_eq!(placement.group, Some(SCRIPT_GROUP | 2404));
        assert!(!placement.dialogue);
        // Its level alone restates nothing of a creature made before.
        let level = State::of(&[op(LEVEL, Value::I32(7))], &[]);
        assert!(!level.restates());
    }

    #[test]
    fn set_icon_changes_the_first_entry_of_its_dialogue() {
        let op = |tag, value| Operand { tag, value };
        let dialog = |name: &str, icon| Record {
            kind: 40,
            body: Body::Dialog {
                name: name.into(),
                values: [11547, icon],
            },
        };
        let set_icon = |ops: Vec<Operand>| Record {
            kind: SET_ICON,
            body: Body::Operands(ops),
        };
        let records = [
            dialog("first", 10),
            dialog("auftrag10", 0),
            dialog("auftrag10", 11),
            // The second `auftrag10` keeps its own; one of no name changes nothing here.
            set_icon(vec![
                op(0x01, Value::Str("AUFTRAG10".into())),
                op(0x0B, Value::I32(13)),
            ]),
            set_icon(vec![op(0x0B, Value::I32(8))]),
            set_icon(vec![
                op(0x01, Value::Str("nowhere".into())),
                op(0x0B, Value::I32(1)),
            ]),
        ];
        let refs: Vec<&Record> = records.iter().collect();
        let (dialogues, mut icons) = dialogue_table(&refs);
        assert_eq!(icons, [10, 10, 0, 11]);
        reicon(&dialogues, &mut icons, set_icons(&records));
        assert_eq!(icons, [10, 10, 13, 11]);
        assert_eq!(dialogues[2], "auftrag10");
    }

    #[test]
    fn the_hires_tags_add_up() {
        use crate::followers::Hire;
        let op = |tag| Operand {
            tag,
            value: Value::I32(0),
        };
        let marker = |tag| Operand {
            tag,
            value: Value::Marker,
        };
        assert_eq!(hire_of(&[op(0x01)]), None);
        assert_eq!(hire_of(&[op(0x4B)]), Some(Hire::Fights));
        assert_eq!(hire_of(&[op(0x37)]), Some(Hire::Escorts));
        assert_eq!(hire_of(&[op(0x71)]), Some(Hire::Escorts));
        // A `0x37` with a `0x4B` hires as `0x4B`; a `0x65` after a `0x37` leaves the hire.
        assert_eq!(hire_of(&[op(0x37), op(0x4B)]), Some(Hire::Fights));
        assert_eq!(hire_of(&[op(0x37), marker(0x65)]), Some(Hire::Escorts));
        assert_eq!(hire_of(&[marker(0x39)]), Some(Hire::LetsGo));
        assert_eq!(hire_of(&[op(0x4B), marker(0x39)]), Some(Hire::LetsGo));
        // A `SetNPCState` of a hire restates a creature made before it.
        assert!(State::of(&[op(0x37)], &[]).restates());
    }

    /// What two soldiers' types are made of: 279 of the class 3, holding the shield 1211, and 276
    /// of the class 7, with the shields 1211 and 1209, and their models' clips.
    fn soldiers() -> (Makers, HashMap<String, motions::Slots>) {
        let item = |slot, name: &str, category: u8| Item {
            slot,
            kind: 2,
            class: 0,
            subclass: 0,
            name: name.into(),
            texture: 0,
            sprite: 0,
            byte_03: 0,
            words_08: [0; 2],
            words_14: [0, 0, 0, 0, 0, 0, u32::from(category) << 16, 0],
            bytes_34: [0; 3],
            byte_57: 0,
            words_58: [0; 10],
        };
        let makers = Makers {
            assets: std::path::PathBuf::new(),
            parents: HashMap::new(),
            items: HashMap::from([
                (279, item(279, "soldier.grn", 0)),
                (276, item(276, "renegade.grn", 0)),
                (1211, item(1211, "shield_a.grn", SHIELD)),
                (1209, item(1209, "shield_b.grn", SHIELD)),
            ]),
            kinds: HashMap::new(),
            models: HashMap::from([
                ("SOLDIER.GRN".to_owned(), "models/soldier.glb".to_owned()),
                ("RENEGADE.GRN".to_owned(), "models/renegade.glb".to_owned()),
                ("SHIELD_A.GRN".to_owned(), "models/shield_a.glb".to_owned()),
                ("SHIELD_B.GRN".to_owned(), "models/shield_b.glb".to_owned()),
            ]),
            textures: HashMap::new(),
            speeds: HashMap::from([(279, [100, 200]), (276, [110, 220])]),
            classes: HashMap::from([(279, 3), (276, 7)]),
            bases: HashMap::new(),
            record_flags: HashMap::new(),
            shadowless: HashSet::new(),
        };
        let slots = HashMap::from([
            ("models/soldier.glb".to_owned(), motions::Slots::default()),
            ("models/renegade.glb".to_owned(), motions::Slots::default()),
        ]);
        (makers, slots)
    }

    #[test]
    fn a_create_obj_of_a_quest_item_lays_it_to_be_picked_up() {
        let (mut makers, mut slots) = soldiers();
        let item = |slot, name: &str, category: u8| Item {
            slot,
            kind: 34,
            class: 1,
            subclass: 8,
            name: name.into(),
            texture: 0,
            sprite: 0,
            byte_03: 0,
            words_08: [0; 2],
            words_14: [0, 0, 0, 0, 0, 0, u32::from(category) << 16, 0],
            bytes_34: [0; 3],
            byte_57: 0,
            words_58: [0; 10],
        };
        makers.items.insert(6001, item(6001, "bush01.grn", 15));
        makers.items.insert(5218, item(5218, "chest.grn", CHEST));
        makers
            .models
            .insert("BUSH01.GRN".to_owned(), "models/bush01.glb".to_owned());
        makers
            .models
            .insert("CHEST.GRN".to_owned(), "models/chest.glb".to_owned());
        let op = |tag, value| Operand { tag, value };
        let mut made = |kind: u32, rest: &[Operand]| {
            let mut ops = vec![
                op(0x01, Value::Str("res:ZIELOBJEKT_15001".into())),
                op(0x02, Value::I32(i32::try_from(kind).expect("a type"))),
                op(0x04, Value::I32x3([3252, 2546, 0])),
            ];
            ops.extend_from_slice(rest);
            let record = Record {
                kind: CREATE_OBJ,
                body: Body::Operands(ops.clone()),
            };
            makers
                .placement(
                    &record,
                    &ops,
                    Origin::default(),
                    (&HashMap::new(), &HashMap::new()),
                    &mut slots,
                )
                .expect("made")
                .expect("placed")
        };
        // Quest 15001's plant: `0x41` `od_15001`, `0x61` and `0x8E` (#1256).
        let take = op(ON_TAKE, Value::Str("od_15001".into()));
        let plant = made(
            6001,
            &[
                take.clone(),
                op(0x61, Value::Marker),
                op(QUEST_ITEM, Value::Marker),
            ],
        );
        assert!(plant.object);
        assert_eq!(plant.quest, Some(Some("od_15001".to_owned())));
        // Without `0x8E` it is no quest's item lying, and a chest, which nobody picks up, none
        // either.
        assert_eq!(made(6001, std::slice::from_ref(&take)).quest, None);
        assert_eq!(
            made(5218, &[take, op(QUEST_ITEM, Value::Marker)]).quest,
            None
        );
        assert!(takeable(5) && takeable(15) && takeable(33));
        assert!(!takeable(4) && !takeable(10) && !takeable(12) && !takeable(26));
    }

    #[test]
    fn a_create_npc_of_a_turned_area_makes_the_lists_type() {
        let (makers, slots) = soldiers();
        let op = |tag, value| Operand { tag, value };
        let made = |rest: &[Operand]| {
            let mut ops = vec![
                op(0x01, Value::Str("NON_UNIQUE".into())),
                op(0x02, Value::I32(279)),
                op(0x04, Value::I32x3([3200, 2500, 0])),
            ];
            ops.extend_from_slice(rest);
            let record = Record {
                kind: CREATE_NPC,
                body: Body::Operands(ops.clone()),
            };
            makers
                .placement(
                    &record,
                    &ops,
                    Origin {
                        enter: true,
                        started: false,
                    },
                    (&HashMap::new(), &HashMap::new()),
                    &mut slots.clone(),
                )
                .expect("made")
                .expect("placed")
        };
        // An `Enter`'s soldier in the guards' mode (`_46`), peaceful, keeps its type: its record
        // has no `0x6B`, as the original's soldiers came again after `GoodToBad` (#1102).
        assert!(!made(&[op(0x46, Value::Marker)]).turns());
        // One of a `0x6B` of 0 takes `GoodToBad`'s pair of area 4, which turns it into 276 in the
        // mode of `0x12`, hostile.
        let soldier = made(&[op(0x46, Value::Marker), op(RISES, Value::I32(0))]);
        assert!(soldier.turns());
        assert!(!minds::hostile_to_heroes(soldier.side, 279));
        let mut lists = quests::AreaTypes::default();
        lists.add(
            4,
            &[279, 276],
            quests::region_change_word([0x08, 0x12]),
            false,
        );
        let turn = lists.turn(4, soldier.creature.0).expect("a pair of 279");
        let turned = makers
            .retype(&soldier, turn, &mut slots.clone())
            .expect("its clips")
            .expect("its model");
        assert_eq!(turned.model, "models/renegade.glb");
        assert_eq!(turned.creature.0, 276);
        assert_eq!(turned.creature.2, 0x1);
        assert_eq!(turned.speed, [110.0, 220.0]);
        assert_eq!(turned.cell, soldier.cell);
        // Its record's hostility none and its new mode 1, it takes the class 7, a friend of the
        // heroes.
        assert_eq!(turned.side, 7);
        assert!(!minds::hostile_to_heroes(turned.side, 276));
        // A trader, and one of a `0x6B` above 0, keep their types.
        assert!(!made(&[op(0x24, Value::Marker), op(RISES, Value::I32(0))]).turns());
        assert!(!made(&[op(RISES, Value::I32(1))]).turns());
        assert!(made(&[op(RISES, Value::I32(0))]).turns());
        // A play's `CreateNPC` takes the pair of its cell's area that `scripts` marked it with.
        let marked = [
            op(0x02, Value::I32(279)),
            op(AREA_TURN, Value::I32x3([4, 276, 1])),
            op(AREA_TURN, Value::I32x3([5, 296, 0])),
        ];
        assert_eq!(
            area_turn(&marked, 4),
            Some(quests::Turn {
                from: 279,
                to: 276,
                flags: 1
            })
        );
        assert_eq!(area_turn(&marked, 5).map(|turn| turn.to), Some(296));
        assert_eq!(area_turn(&marked, 6), None);
    }

    #[test]
    fn a_region_change_makes_its_areas_soldier_hostile_of_the_new_type_with_its_items() {
        let (makers, mut slots) = soldiers();
        let op = |tag, value| Operand { tag, value };
        let ops = vec![
            op(0x01, Value::Str("NON_UNIQUE".into())),
            op(0x02, Value::I32(279)),
            op(0x02, Value::I32(1211)),
            op(0x04, Value::I32x3([3200, 2500, 0])),
            op(0x46, Value::Marker),
        ];
        let record = Record {
            kind: CREATE_NPC,
            body: Body::Operands(ops.clone()),
        };
        let soldier = makers
            .placement(
                &record,
                &ops,
                Origin::default(),
                (&HashMap::new(), &HashMap::new()),
                &mut slots,
            )
            .expect("made")
            .expect("placed");
        assert_eq!(soldier.items, [1211]);
        assert_eq!(soldier.equipment[0].model, "models/shield_a.glb");
        assert!(soldier.changes());
        // `GoodToBad`'s record of area 4: the soldier, its shield, its word and its side.
        let change = quests::AreaChange {
            area: 4,
            types: vec![279, 276, 1211, 1209],
            word: quests::region_change_word([0x08, 0x12]),
            hostile: Some(true),
        };
        let changed = makers
            .change(&soldier, &change, &mut slots)
            .expect("its clips")
            .expect("turned");
        assert_eq!(changed.creature.0, 276);
        assert_eq!(changed.model, "models/renegade.glb");
        assert_eq!(changed.items, [1209]);
        assert_eq!(changed.equipment[0].model, "models/shield_b.glb");
        assert_eq!(changed.creature.2, 0x1);
        assert_eq!(changed.side, 2);
        assert!(minds::hostile_to_heroes(changed.side, 276));
        // A type of none of its pairs, and a trader, are not changed.
        assert!(
            makers
                .change(&changed, &change, &mut slots)
                .expect("read")
                .is_none()
        );
        let trader = Placement {
            trades: quests::MERCHANT,
            ..soldier.clone()
        };
        assert!(!trader.changes());
        // A creature's health keeps its share as its most is made anew.
        let rules = fight::Rules::default();
        let kept = health_kept(
            fight::Health {
                current: 50,
                most: 200,
                level: 5,
            },
            &changed,
            &rules,
        );
        assert_eq!(kept.level, 5);
        assert_eq!(kept.current, kept.most / 4);
    }

    #[test]
    fn a_creature_or_an_object_made_at_a_creature_takes_its_place() {
        let op = |tag, value| Operand { tag, value };
        let made = |kind, at: &str| Record {
            kind,
            body: Body::Operands(vec![
                op(0x02, Value::I32(697)),
                op(0x04, Value::Name(at.into())),
            ]),
        };
        let mut records = [
            made(CREATE_NPC, "CPOS:HERO"),
            made(CREATE_OBJ, "CPOS:RES:17190"),
            made(CREATE_NPC, "CPOS:res:404"),
            made(CREATE_NPC, "posAhnen"),
        ];
        locate(&mut records, |name| match name {
            None => Some((3218, 2514, 1)),
            Some("res:17190") => Some((3240, 2520, 0)),
            Some(_) => None,
        });
        let at = |record: &Record| match &record.body {
            Body::Operands(ops) => ops[1].value.clone(),
            _ => unreachable!("made with operands"),
        };
        assert_eq!(at(&records[0]), Value::I32x3([3218, 2514, 1]));
        assert_eq!(at(&records[1]), Value::I32x3([3240, 2520, 0]));
        // A creature gone, and a `DefPos`'s name, keep theirs.
        assert_eq!(at(&records[2]), Value::Name("CPOS:res:404".into()));
        assert_eq!(at(&records[3]), Value::Name("posAhnen".into()));
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
        placements[2].trades = quests::MERCHANT;
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
        assert_eq!(placements[2].trades, 0);
        assert!(!placements[2].dialogue);
        assert_eq!(placements[2].creature.2, ways::flags_word([0x24]));
        assert_eq!(placements[3].trades, quests::TRADER);
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
    }

    #[test]
    fn a_sector_fills_anew_once_another_region_filled() {
        let stood = |x| Stood {
            cell: (x, 2518),
            layer: 0,
            health: fight::Health {
                current: 30,
                most: 30,
                level: 2,
            },
        };
        let rabbit = |at| Placed {
            at,
            source: Source::Filled(Box::new(Placement {
                model: "rabbit".to_owned(),
                ..Placement::default()
            })),
        };
        // The sectors of x 49 and more are of the region 2, the others of the region 3.
        let region_of = |(x, _): (u32, u32)| if x >= 49 { 2 } else { 3 };
        // How many creatures come back, and whether the sector fills anew.
        let enter = |kept: &mut Kept, at: (u32, u32)| {
            let entered = kept.enter(at, Some(region_of(at)), region_of);
            (entered.back.len(), entered.fill)
        };
        let (here, there) = ((49, 39), (47, 39));
        let mut kept = Kept::default();
        // Filled at first; a sector of no fills fills nothing.
        assert_eq!(enter(&mut kept, here), (0, true));
        assert!(!kept.enter((50, 39), None, region_of).fill);
        // One of its fills went living: it comes back where it stood, and the sector fills
        // nothing.
        kept.gone(&rabbit(here), Some(stood(3150)));
        let entered = kept.enter(here, Some(2), region_of);
        assert!(!entered.fill);
        assert_eq!(entered.back.len(), 1);
        assert_eq!(entered.back[0].1, stood(3150));
        // Killed: the sector stays without its fills while its region filled last.
        kept.gone(&rabbit(here), None);
        assert_eq!(enter(&mut kept, here), (0, false));
        assert_eq!(enter(&mut kept, here), (0, false));
        // Another region's sector fills, dropping what the region 2 kept and its time; back in
        // the region 2, the sector fills anew, and so does one whose creature had been kept.
        let near = (50, 40);
        kept.gone(&rabbit(near), Some(stood(3205)));
        assert_eq!(enter(&mut kept, there), (0, true));
        assert_eq!(enter(&mut kept, here), (0, true));
        assert_eq!(enter(&mut kept, near), (0, true));
        // A sector of the region filled last that never kept a time fills anew at each coming;
        // one of the region 3 that kept a creature while the region 2 filled last fills anew as
        // it comes back, dropping it.
        kept.gone(&rabbit(there), Some(stood(3010)));
        assert_eq!(enter(&mut kept, (49, 41)), (0, true));
        assert_eq!(enter(&mut kept, (49, 41)), (0, true));
        assert_eq!(enter(&mut kept, there), (0, true));
    }

    #[test]
    fn a_play_anim_names_its_creature_and_slot_and_the_others_take_their_turns() {
        let op = |tag, value| Operand { tag, value };
        let name = |n: &str| op(0x01, Value::Str(n.into()));
        let slot = |n| op(0x0B, Value::I32(n));
        // `OMO-1malorka_dance`'s first, under the anim mode.
        let clip = played(PLAY_ANIM, &[name("res:17532"), slot(53)], true, 0.0).unwrap();
        assert_eq!(clip.name, "res:17532");
        assert_eq!(clip.act, Act::Clip { slot: 53, loops: 1 });
        assert!(clip.queued);
        // A second `0x0B` its count; the name taken case aside.
        let twice = played(PLAY_ANIM, &[name("Malorka"), slot(89), slot(2)], false, 0.0).unwrap();
        assert_eq!(
            (twice.name.as_str(), twice.act),
            ("malorka", Act::Clip { slot: 89, loops: 2 })
        );
        // None for the hero, the smith's window or no slot.
        assert!(played(PLAY_ANIM, &[name("HERO"), slot(53)], true, 0.0).is_none());
        assert!(played(PLAY_ANIM, &[name("schmiede"), slot(53)], true, 0.0).is_none());
        assert!(played(PLAY_ANIM, &[name("res:17532"), slot(0)], true, 0.0).is_none());
        // A `CallFunktion` that the scripts deferred names its function first and its creature
        // second, and always waits its turn (#1278).
        let call = [name("sprich_15"), name("res:17532")];
        let turn = played(22, &call, false, 0.0).unwrap();
        assert_eq!(
            (turn.name.as_str(), turn.act, turn.queued),
            ("res:17532", Act::Call("sprich_15".into()), true)
        );
        // A `Wait` waits its turn only under the mode: on another creature or the hero, or
        // for a time of the real clock from the record's whole second.
        let until = op(0x5C, Value::Marker);
        let wait = played(
            90,
            &[name("res:17273"), name("HERO"), until.clone()],
            true,
            0.0,
        );
        assert_eq!(
            wait.map(|wait| (wait.name, wait.act)),
            Some(("res:17273".into(), Act::Wait(None, Until::Busy)))
        );
        assert!(played(90, &[name("res:17273"), name("HERO"), until], false, 0.0).is_none());
        let seconds = played(
            90,
            &[name("res:17276"), op(0x5F, Value::I32(3))],
            true,
            12.7,
        );
        assert_eq!(
            seconds.map(|wait| wait.act),
            Some(Act::Wait(None, Until::Time(15.0)))
        );
    }

    #[test]
    fn an_npc_goto_walks_its_last_name_and_a_teleport_of_none_puts_the_hero_in_its_turn() {
        let op = |tag, value| Operand { tag, value };
        let name = |n: &str| op(0x01, Value::Str(n.into()));
        let cell = op(0x04, Value::I32x3([3986, 1813, 0]));
        // `btn_Deamonstart`'s walk names the hero and then Anducar: Anducar's, of no cell.
        let start = [name("HERO"), name("res:Anducar01"), op(0x66, Value::Marker)];
        assert_eq!(
            mover(NPC_GOTO, &start, true).as_deref(),
            Some("res:anducar01")
        );
        // `annehmen2003`'s: the hero's, under the mode or not; none without a name.
        assert_eq!(
            mover(NPC_GOTO, &[name("HERO"), cell.clone()], true).as_deref(),
            Some(HERO)
        );
        assert_eq!(
            mover(NPC_GOTO, &[name("HERO"), cell.clone()], false).as_deref(),
            Some(HERO)
        );
        assert_eq!(mover(NPC_GOTO, std::slice::from_ref(&cell), true), None);
        // A teleport of no creature or of the hero is the hero's in its queue, the scripts'
        // outside the mode; another creature's either way.
        assert_eq!(
            mover(TELEPORT, std::slice::from_ref(&cell), true).as_deref(),
            Some(HERO)
        );
        assert_eq!(mover(TELEPORT, std::slice::from_ref(&cell), false), None);
        assert_eq!(mover(TELEPORT, &[name("hero"), cell.clone()], false), None);
        assert_eq!(
            mover(TELEPORT, &[name("res:18103"), cell], false).as_deref(),
            Some("res:18103")
        );
        assert_eq!(mover(PLAY_ANIM, &[name("HERO")], true), None);
    }

    #[test]
    fn a_wait_holds_while_its_condition_holds() {
        // Until the other stands idle: while it walks or fights, and done where it is gone.
        assert!(Until::Busy.holds(Some((false, false)), 0.0));
        assert!(!Until::Busy.holds(Some((false, true)), 0.0));
        assert!(!Until::Busy.holds(None, 0.0));
        assert!(Until::Fights.holds(Some((true, false)), 0.0));
        assert!(Until::Lives.holds(Some((false, true)), 0.0));
        assert!(!Until::Lives.holds(None, 0.0));
        // A time: three seconds from the record's whole second; hundredths where negative.
        let time = Until::of(0x5F, 3, 12.7).unwrap();
        assert!(time.holds(None, 14.9) && !time.holds(None, 15.0));
        let hundredths = Until::of(0x5F, -50, 12.731).unwrap();
        assert!(hundredths.holds(None, 13.22) && !hundredths.holds(None, 13.23));
        // The mode, which no span keeps past its run, and the variables' bits hold nothing.
        assert!(!Until::Mode.holds(None, 0.0));
        assert!(!Until::Bit.holds(None, 0.0));
        assert!(Until::Busy.stops() && !time.stops());
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
            scripted: HashMap::new(),
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
    fn a_creatures_strike_reaches_three_times_its_reach_with_its_target() {
        // A goblin warrior (76) against the Seraphim (100) and the Gladiator (120), and a great
        // beast (52, 480) against the Seraphim (#677).
        let goblin = minds::size(76, false);
        assert_eq!(hits_within(goblin, minds::size(1, true)), 345.0);
        assert_eq!(hits_within(goblin, minds::size(2, true)), 375.0);
        assert_eq!(
            hits_within(minds::size(52, false), minds::size(1, true)),
            870.0
        );
        assert_eq!(hits_within(goblin, goblin), 390.0);
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
    fn the_start_places_the_npcs_of_the_quests_sections_it_ran() {
        let op = |tag, value| Operand { tag, value };
        let record = |kind, ops| Record {
            kind,
            body: Body::Operands(ops),
        };
        // The start as the scripts ran it (`scripts::run`): quest 15004's giver of its `OnSetUp`
        // after its set-up, quest 1's novice of its `OnEnter`, sent to the chapel's benches.
        let start = vec![
            record(
                DEF_POS,
                vec![
                    op(0x01, Value::Str("POS_AUFTRAG_15004".into())),
                    op(0x0B, Value::I32(3370)),
                    op(0x0B, Value::I32(2524)),
                ],
            ),
            record(21, vec![op(0x0B, Value::I32(15004))]),
            record(
                CREATE_NPC,
                vec![
                    op(0x01, Value::Str("res:AUFTRAGGEBER_15004".into())),
                    op(0x02, Value::I32(288)),
                    op(0x04, Value::Name("POS_AUFTRAG_15004".into())),
                    op(0x0E, Value::Marker),
                ],
            ),
            record(
                CREATE_NPC,
                vec![
                    op(0x01, Value::Str("res:17095".into())),
                    op(0x02, Value::I32(679)),
                    op(0x04, Value::Name("novizin1".into())),
                ],
            ),
            record(
                NPC_GOTO,
                vec![
                    op(0x01, Value::Str("Res:17095".into())),
                    op(0x04, Value::I32x3([3237, 2514, 0])),
                ],
            ),
        ];
        let sector = Function {
            name: "Sector5039Enter".into(),
            quest: -1,
            region: 0,
            records: vec![record(SPAWN_VALUES, vec![op(0x0B, Value::I32(50))])],
        };
        let tree = Tree {
            start,
            quest: Vec::new(),
            functions: vec![sector],
            quests: Vec::new(),
            regions: Vec::new(),
        };
        let records = start_records(&tree);
        let kinds: Vec<u16> = records.iter().map(|r| r.kind).collect();
        assert_eq!(
            kinds,
            [DEF_POS, 21, CREATE_NPC, CREATE_NPC, NPC_GOTO, SPAWN_VALUES]
        );
        // The giver where its `DefPos` puts it; the novice where the quest sends her, whatever
        // the case of her name.
        let Body::Operands(ops) = &records[2].body else {
            panic!("the giver has operands");
        };
        let named = positions(records.iter().copied());
        assert_eq!(position(ops, &named), Some((3370, 2524, 0)));
        let targets = goto_targets(&records);
        assert_eq!(targets["res:17095"], (3237, 2514, 0));
    }

    #[test]
    fn a_createnpc_and_its_setnpcstates_give_the_creature_its_deaths_functions() {
        let op = |tag, value| Operand { tag, value };
        let text = |tag, name: &str| op(tag, Value::Str(name.into()));
        // `CreateNPC`'s `0x05` and `0x60`, each added in its order.
        let mut functions = death_functions(&[
            text(0x01, "res:17260"),
            text(ON_DEATH, "tot2203"),
            text(ON_SIGHT, "schmuggler1402"),
        ]);
        let mut copy = functions.clone();
        assert_eq!(copy.take().as_deref(), Some("tot2203"));
        // A `SetNPCState`'s `0x05` replaces them, its `0x60` adds one after.
        let state = State::of(
            &[
                text(0x01, "res:17260"),
                text(ON_DEATH, "lose670"),
                text(ON_SIGHT, "od_5002"),
            ],
            &[],
        );
        state.restate(&mut functions);
        assert_eq!(functions.take().as_deref(), Some("lose670"));
        assert!(functions.is_empty());
    }

    #[test]
    fn the_start_position_gives_its_cell_and_its_floor_where_it_names_one() {
        let op = |tag, value| Operand { tag, value };
        let tree = |ops: Vec<Operand>| Tree {
            start: vec![Record {
                kind: START_POSITION,
                body: Body::Operands(ops),
            }],
            quest: Vec::new(),
            functions: Vec::new(),
            quests: Vec::new(),
            regions: Vec::new(),
        };
        let cell = |v| op(0x0B, Value::I32(v));
        // The Vampiress's, on floor 2; the Gladiator's, of no floor.
        let vampiress = tree(vec![cell(3500), cell(2477), cell(2)]);
        assert_eq!(start_position(&vampiress), Some(((3500, 2477), Some(2))));
        let gladiator = tree(vec![cell(3790), cell(349)]);
        assert_eq!(start_position(&gladiator), Some(((3790, 349), None)));
    }

    #[test]
    fn a_set_hp_gives_its_creature_a_share_of_its_most_health() {
        let op = |tag, value| Operand { tag, value };
        let record = |kind, ops| Record {
            kind,
            body: Body::Operands(ops),
        };
        // Quest 201's horse in `Region2Init` (#1001), and a share past 100 held to it.
        let records = [
            record(
                CREATE_NPC,
                vec![
                    op(0x01, Value::Str("Res:18007".into())),
                    op(0x02, Value::I32(551)),
                ],
            ),
            record(
                SET_HP,
                vec![
                    op(0x01, Value::Str("res:18007".into())),
                    op(0x0B, Value::I32(50)),
                ],
            ),
            record(
                CREATE_NPC,
                vec![
                    op(0x01, Value::Str("strong".into())),
                    op(0x02, Value::I32(551)),
                ],
            ),
            record(
                SET_HP,
                vec![
                    op(0x01, Value::Str("strong".into())),
                    op(0x0B, Value::I32(250)),
                ],
            ),
        ];
        let made = HashMap::from([
            (std::ptr::from_ref(&records[0]), 0),
            (std::ptr::from_ref(&records[2]), 1),
        ]);
        let records: Vec<&Record> = records.iter().collect();
        let mut placements = vec![Placement::default(), Placement::default()];
        set_states(&records, &made, &mut placements);
        assert_eq!(placements[0].health_share, Some(50));
        assert_eq!(placements[1].health_share, Some(100));
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
            // The chests' trigger, closed; a door's, closed then opened; one of neither, which the
            // client does not make; a locked door's, closed (#1006); an unlocked one's, open.
            create("chestTrigger", &[CLOSED]),
            create("door", &[CLOSED, OPEN]),
            create("plain", &[]),
            create("R24NQ11_Tuer", &[LOCKED]),
            create("unlocked", &[OPEN, LOCKED, UNLOCKED, OPEN]),
            // A wide chest's three cells on floor 1, and a patch of each other trigger.
            patch(
                "chestTrigger",
                &[[3341, 2512, 1], [3342, 2512, 1], [3343, 2512, 1]],
            ),
            patch("door", &[[3300, 2500, 0]]),
            patch("plain", &[[3301, 2500, 0]]),
            patch("CHESTTRIGGER", &[[3200, 2490, 0]]),
            patch("R24NQ11_Tuer", &[[2400, 3000, 0]]),
            patch("unlocked", &[[2401, 3000, 0]]),
        ];
        let records: Vec<&Record> = records.iter().collect();
        let chest = |x, y, floor| ("chesttrigger".to_owned(), (x, y, floor));
        let states = trigger_states(&records);
        let closed: Vec<(String, Spot)> = trigger_cells(&records)
            .into_iter()
            .filter(|(name, _)| !states[name].0)
            .collect();
        assert_eq!(
            closed,
            [
                chest(3341, 2512, 1),
                chest(3342, 2512, 1),
                chest(3343, 2512, 1),
                chest(3200, 2490, 0),
                ("r24nq11_tuer".to_owned(), (2400, 3000, 0)),
            ]
        );
        assert_eq!(states["r24nq11_tuer"], (false, true));
        assert_eq!(states["unlocked"], (true, false));
        assert_eq!(states["door"], (true, false));
        assert!(!states.contains_key("plain"));
    }

    #[test]
    fn a_script_name_of_the_prefix_res_alone_is_a_text_name() {
        let of = |key| TextName::of(key).map(|name| name.0);
        assert_eq!(
            of("res:zielobjekt_15001").as_deref(),
            Some("zielobjekt_15001")
        );
        assert_eq!(of("RES:17640").as_deref(), Some("17640"));
        assert_eq!(of("non_unique"), None);
        assert_eq!(of("gitter1_107"), None);
        assert_eq!(of("rés"), None);
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
        // The regions' `Init` functions run with the sectors' (#585).
        assert!(of_sectors("Sector50039Enter") && of_sectors("Region2Init"));
        assert!(of_sectors("Region111Init"));
        assert!(!of_sectors("Region2Enter") && !of_sectors("RegionInit"));
        assert!(!of_sectors("QIS_OnEnter1"));
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
    fn a_hero_wears_what_it_neither_holds_nor_carries_on_a_bone() {
        let gear = |category, model: &str| Gear {
            category,
            kind: None,
            model: Some(model.to_owned()),
            texture: None,
            counts: false,
            sound: 0,
            size: 50.0,
        };
        // A sword in the hand, a hood, a ring and wings for the back.
        let gear: HashMap<u32, Gear> = [
            (7904, gear(WEAPON, "staff.glb")),
            (3219, gear(17, "cowl.glb")),
            (1001, gear(8, "ring.glb")),
            (4006, gear(25, "wings.glb")),
        ]
        .into_iter()
        .collect();
        let worn: Vec<Worn> = [7904, 3219, 1001, 4006]
            .into_iter()
            .map(|item| Worn {
                id: 0,
                item,
                words: Vec::new(),
            })
            .collect();
        let holding = InHand::of([(7904, WEAPON, None)]);
        let wears = worn_gear(&gear, &worn, &holding);
        assert_eq!(
            wears
                .iter()
                .map(|held| held.model.as_str())
                .collect::<Vec<_>>(),
            ["cowl.glb"]
        );
        assert_eq!(wears[0].mount, Mount::Worn);
    }

    #[test]
    fn a_figure_shows_its_items_by_their_slots() {
        let gear = |category, kind, model: &str| Gear {
            category,
            kind,
            model: Some(model.to_owned()),
            texture: None,
            counts: false,
            sound: 0,
            size: 50.0,
        };
        let gear: HashMap<u32, Gear> = [
            (1242, gear(WEAPON, Some(1), "sword.glb")),
            (1246, gear(SHIELD, None, "shield.glb")),
            (1220, gear(17, None, "armour.glb")),
            (4006, gear(25, None, "wings.glb")),
            (1001, gear(8, None, "ring.glb")),
        ]
        .into_iter()
        .collect();
        // The sword in the right hand, the shield on the left forearm, the armour worn, the wings
        // on the back; a ring's slot 9 is none of the figure's, and 4054 has no gear.
        let (class, held) = showcase(
            &gear,
            &[
                (13, 1242),
                (12, 1246),
                (1, 1220),
                (17, 4006),
                (9, 1001),
                (2, 4054),
            ],
        );
        let shown: Vec<(&str, Hand, Mount)> = held
            .iter()
            .map(|held| (held.model.as_str(), held.hand, held.mount))
            .collect();
        assert_eq!(
            shown,
            [
                ("sword.glb", Hand::Right, Mount::Hand),
                ("shield.glb", Hand::Left, Mount::Forearm),
                ("armour.glb", Hand::Right, Mount::Worn),
                ("wings.glb", Hand::Right, Mount::Back),
            ]
        );
        assert_eq!(class, motions::class(Some((true, 0)), Some(1)));
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
                WEAPON_POOL,
                vec![op(0x0B, Value::I32(pool)), op(0x02, Value::I32(item))],
            )
        };
        let records = [
            pool(125, 1811),
            pool(125, 1812),
            pool(125, 1812),
            pool(134, 1729),
            pool(50, 1203),
            pool(124, 1731),
        ];
        let records: Vec<&Record> = records.iter().collect();
        let pools = pools(&records);
        assert_eq!(pools[&125], [1811, 1812, 1812]);
        // Thieves: always a bow from 125, a shield never after a bow; a sword's wielder 304
        // takes the shield by its chance; 307 a crossbow from 124.
        let rule = |types: Vec<u32>, items| Rule { types, items };
        let rules = [
            rule(vec![306, 313], [(125, 100), (50, 100), (-1, 0)]),
            rule(vec![304], [(134, 100), (50, 100), (-1, 0)]),
            rule(vec![306], [(134, 100), (0, 0), (-1, 0)]),
            rule(vec![307], [(124, 100), (50, 100), (-1, 0)]),
        ];
        let gear = |category, kind| Gear {
            category,
            kind,
            model: Some("m.glb".into()),
            texture: None,
            counts: true,
            sound: 0,
            size: 50.0,
        };
        let gear: HashMap<u32, Gear> = [
            (1811, gear(WEAPON, Some(BOW))),
            (1812, gear(WEAPON, Some(BOW))),
            (1729, gear(WEAPON, Some(1))),
            (1203, gear(SHIELD, None)),
            (1731, gear(WEAPON, Some(CROSSBOW))),
        ]
        .into();
        let mut rng = spawning::Rng::of(48, 39, 0);
        let kinds = |items: Vec<(u32, &Gear)>| {
            items
                .iter()
                .map(|&(item, g)| (item, g.category, g.kind))
                .collect::<Vec<_>>()
        };
        let shots = |crossbow, bow| spawning::Shots { crossbow, bow };
        // The weapons' choice: the bow 1812 or the sword 1729 by the kind asked.
        let chosen = |kind, _: &mut spawning::Rng| match kind {
            spawning::BOW => Some(1812),
            _ => Some(1729),
        };
        let mut items = |creature, shots, weapon: fn(u8, &mut spawning::Rng) -> Option<u32>| {
            kinds(gear_of(
                &rules,
                creature,
                (&pools, &gear),
                (shots, weapon),
                &mut rng,
            ))
        };
        // The first rule that names 306 wins: a bow alone, of the pool's at random.
        assert_eq!(
            items(306, shots(true, true), chosen),
            [(1812, WEAPON, Some(BOW))]
        );
        assert_eq!(
            items(304, shots(true, true), chosen),
            [(1729, WEAPON, Some(1)), (1203, SHIELD, None)]
        );
        // A type no rule names takes nothing.
        assert!(items(516, shots(true, true), chosen).is_empty());
        // A crossbow (#1217): kept by a model that shoots it; a thief's model, which shoots a bow
        // alone, takes a bow and so no shield; a model that shoots neither takes a sword, and the
        // shield after it; none chosen, nothing.
        assert_eq!(
            items(307, shots(true, true), chosen),
            [(1731, WEAPON, Some(CROSSBOW))]
        );
        assert_eq!(
            items(307, shots(false, true), chosen),
            [(1812, WEAPON, Some(BOW))]
        );
        assert_eq!(
            items(307, shots(false, false), chosen),
            [(1729, WEAPON, Some(1)), (1203, SHIELD, None)]
        );
        assert!(items(307, shots(false, true), |_, _| None).is_empty());
    }

    #[test]
    fn a_created_creature_takes_weapons_its_model_shoots() {
        // The crossbow 1731, the bow 1812, the sword 1729 and a shield 1203 of no kind.
        let kinds: HashMap<u32, u8> = [(1731, CROSSBOW), (1812, BOW), (1729, 1)].into();
        let shots = |crossbow, bow| spawning::Shots { crossbow, bow };
        let chosen = |kind| Some(if kind == spawning::BOW { 1812 } else { 1729 });
        // A thief of 54,39 (`create_npc { _02 = { 260, 1730 } }`): its crossbow becomes a bow.
        assert_eq!(
            shootable(&[1731, 1203], &kinds, shots(false, true), chosen),
            [1812, 1203]
        );
        // A model that shoots neither: the crossbow a bow, the bow a sword; a bow alone too.
        assert_eq!(
            shootable(&[1731], &kinds, shots(false, false), chosen),
            [1729]
        );
        assert_eq!(
            shootable(&[1812], &kinds, shots(false, false), chosen),
            [1729]
        );
        // A soldier's model shoots both: kept.
        assert_eq!(
            shootable(&[1731, 1812], &kinds, shots(true, true), chosen),
            [1731, 1812]
        );
        // None chosen: the item is left out.
        assert_eq!(
            shootable(&[1731, 1203], &kinds, shots(false, true), |_| None),
            [1203]
        );
    }
}
