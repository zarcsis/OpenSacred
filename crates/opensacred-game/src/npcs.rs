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
//!
//! The module carries the creatures and the objects on through the play as well: the records of
//! the world that the scripts run during it are placed as they run (`script_records`,
//! `Npcs::add`), with their `SetNPCState`s, `SetGroupState`s, `DelNPC`s, `EquipNPC`s, `SetHP`s,
//! `SetObjState`s, `RegionChange`s and `Morph`s, and under the anim mode a creature's records
//! wait their turns in its queue (`ScriptQueue`, `take_steps`; docs/re/netscript.md, "The anim
//! mode and the creatures' queues"); the creatures walk, strike and die, the bosses and champions
//! level at their first fight, and a killer earns its victim's experience (`walk`, `strike`,
//! `die`, `first_fight`, `earn`); the doors swing (`swing`); a sector keeps its creatures as it
//! goes and puts them back as it comes in (`Kept`, `keep`; docs/re/spawning.md, "The sectors'
//! functions"); a fill's black horse brings its rider (`black_rider`) and the waters their crabs
//! and fish (`swimmers`); the command `spawn` makes creatures (`spawn_creatures`); the hero and
//! the figures of its choice hold and wear their items as an NPC does (`Npcs::hero_gear`,
//! `figure_gear`, `equip`, `bind_worn`); and a save keeps the scripts' placements, the creatures
//! and the hero's followers (`Saved`, `Creatures`, `Following`).

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::ecs::entity::MapEntities;
use bevy::ecs::query::Or;
use bevy::gltf::{Gltf, GltfNode};
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::math::{Affine3A, Quat, Vec3Swizzles};
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::{
    AnimationClip, AnimationGraph, AnimationGraphHandle, AnimationNodeIndex, AnimationPlayer,
    AnimationTransitions, App, AssetId, AssetServer, Assets, ChildOf, Children, Commands,
    Component, DetectChangesMut, Entity, GlobalTransform, Handle, Has, Image, In,
    IntoScheduleConfigs, MeshMaterial3d, MessageReader, MessageWriter, Name, On, Plugin, Query,
    Remove, Res, ResMut, Resource, StandardMaterial, Time, Transform, Update, Vec2, Vec3, With,
    Without, WorldAssetRoot,
};
use bevy::world_serialization::{WorldAsset, WorldInstanceReady};
use opensacred_data::heroes::Worn;
use opensacred_data::netscript::{Body, Record, Tree, Value};
use opensacred_data::tables::{Creature, Item};
use opensacred_data::world::SIDE;
use opensacred_rules::equipment::Switched;
use opensacred_rules::fight::Effect;
use opensacred_rules::quests::DeathFunctions;
use opensacred_rules::shaddar::SHADDAR;
use opensacred_rules::{equipment, minds, quests, script_traps, ways};

use crate::files;
use crate::frame::{Step, WorldTick};
use crate::names::{Id, Named};
use crate::sectors::Streamer;
use crate::tables::Tables;
use crate::terrain::{Floored, Heights, Level};
use crate::{
    ai, arrows, buildings, crowd, daynight, drops, fight, hero, iso, models, motions, paths,
    sectors, sfx, shadows, spawning,
};

/// Record types of the scripts (`opensacred_data::netscript::KEYWORDS`).
const CREATE_NPC: u16 = 1;
const CREATE_OBJ: u16 = 8;
/// The record that puts items on a creature (`EquipNPC`, `0x481D80`; #2752).
const EQUIP_NPC: u16 = 7;
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
/// The tag of `CreateNPC` that makes its creature peaceful whatever comes before and plays its
/// clip of [`EMERGING`] once (`0x4827EA`, `0x4833CE`: the standing action's sub-state `0x65`; the
/// event `0x109` of the state 13, a glow, and `+0x200` bit `0x10` not followed; #2461).
const EMERGES: u8 = 0x8D;
/// The tag of `CreateNPC` and `SetNPCState` that marks its creature a vision, as the creature
/// maker marks the types of `opensacred_rules::spells::VISIONS` (the object's `+0x14` bit
/// `0x200000`, `0x48290F`, `0x4836BE`; docs/re/drawing.md, "The visions"; #3064).
const VISION: u8 = 0xA1;
/// The tag of `SetNPCState` that lets its creature's body go (`0x46224D`–`0x4622B6`, #3445).
const RELEASES: u8 = 0x80;
/// The tag of `SetNPCState` that shows white butterflies over its creature's head for its seconds
/// (`0x462180`, #3446).
const BUTTERFLIES: u8 = 0x7F;
/// The operand of `CreateNPC` whose value of 0 or more arms its creature by the `MakeGroup` rules
/// that the last `Enter` left, as a fill's (`sdk_CREATENPC` at `0x48488E`, `0x47F8E0`; #3072).
const BY_RULES: u8 = 0x54;
/// The clip's slot that [`EMERGES`] plays, at the rate 1.0, once.
const EMERGING: u16 = 154;
/// The big demon (`TYPE_NPC_BIG_DEMON`), which turns hostile as its clip of [`EMERGING`] ends
/// (`0x544EAE`): its side 2, its flags word's bit 0.
const BIG_DEMON: u32 = 53;
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
/// word's bit `0x400000`, by which the creature's first sight of the hero runs it
/// ([`minds::SIGHT`], #2475).
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
/// `NPC_Goto`'s tag of a walk at a walk: its count of orders starts at 1 (`0x49E210`, #1892).
const STROLLS: u8 = 0x66;
/// The records that turn the scripts' anim mode on and off (`SetAnimMode`, `SetCinemaMode`,
/// `SetPlayMode`; the mode word `0xAB73DC`), under which a creature's walks and teleports wait
/// their turns in its queue ([`ScriptQueue`], #1220).
pub(crate) const SET_ANIM_MODE: u16 = 88;
const SET_PLAY_MODE: u16 = 89;
const SET_CINEMA_MODE: u16 = 134;
/// `PlayAnim` (`0x4A2550`): a creature plays a clip of its model's slot (#1277).
const PLAY_ANIM: u16 = 91;
/// `PlaySound` (`0x4A9730`): under the anim mode, its sound waits its turn in the queue of the
/// creature of its second `0x01` (#1826).
const PLAY_SOUND: u16 = 104;
/// `Morph` (`0x4A15A0`): a creature's type replaced (#1829).
const MORPH: u16 = 79;
/// `Partikel` (`0x4A8BB0`): under the anim mode, its effect waits its turn in the queue of the
/// creature of its second `0x01` (the kind 13, #1890).
const PARTIKEL: u16 = 99;
/// `Attack` (`0x4A2B40`): a creature or a group sent at a creature, the hero or a group (#1830).
const ATTACK: u16 = 92;
/// `Attack`'s art or spell by name (`+0xA880`), which #1831 follows; `ECS_NOSPELL` is none.
const ART: u8 = 0x67;
const NO_SPELL: &str = "ecs_nospell";
/// The art of an `Attack` that teleports its creature, `PHASEN` (the reader's 1, #1831).
const TELEPORTS: &str = "ecs_teleport";

/// The spells that the records' reader gives the names of [`ART`] (`0x473306` on), each name
/// whole and case aside; `ECS_NOSPELL` is −1, whose low byte a creature keeps (#1869).
const SPELL_NAMES: [(&str, u8); 27] = [
    ("ecs_teleport", 1),
    ("ecs_superfluid", 4),
    ("ecs_firestrike", 5),
    ("ecs_pentagrammkiller", 6),
    ("ecs_kugelblitz", 8),
    ("ecs_firecircle", 11),
    ("ecs_groundpoison", 12),
    ("ecs_lightning_strike", 13),
    ("ecs_freeze", 14),
    ("ecs_holylight", 15),
    ("ecs_holyshield", 22),
    ("ecs_meteor", 26),
    ("ecs_twister", 28),
    ("ecs_windstrike", 29),
    ("ecs_flash", 33),
    ("ecs_icebullets", 34),
    ("ecs_circleofice", 35),
    ("ecs_changeling", 36),
    ("ecs_bubbletrap", 38),
    ("ecs_killerplants", 39),
    ("ecs_plantcage", 40),
    ("ecs_lostsouls", 41),
    ("ecs_naturehealing", 42),
    ("ecs_reanimate", 43),
    ("ecs_healing", 7),
    ("ecs_fireskin", 24),
    (NO_SPELL, u8::MAX),
];
/// The summon's name, which the reader takes by its first 13 letters, a type after a space
/// kept apart (`+0x261`): the spell 3.
const SUMMONS: &str = "ecs_bigsummon";
/// What the reader gives a name it does not know, which it starts from (`0x4732B0`).
const UNKNOWN_SPELL: u8 = 0xFF;

/// The spell that a creature keeps of a `CreateNPC`'s [`ART`] named `name` (`+0x2B6`, the low
/// byte of the reader's value, `0x483D0A`): [`SPELL_NAMES`]', the summon's 3, else 255, which a
/// magician's chooser takes for a spell of none (`casts::mage_spell`).
fn script_spell(name: &str) -> u8 {
    if name
        .get(..SUMMONS.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(SUMMONS))
    {
        return 3;
    }
    SPELL_NAMES
        .iter()
        .find(|(known, _)| name.eq_ignore_ascii_case(known))
        .map_or(UNKNOWN_SPELL, |&(_, spell)| spell)
}
/// The operand that `npcs` adds to a `Morph` that waited its turn in its creature's queue as it
/// hands it to the world's records ([`take_steps`]): its level and health kept (the step
/// `0x59F266`); past the scripts' last tag, beside [`AREA_TURN`].
const QUEUED_MORPH: u8 = 0xF1;
/// `ActObj` (`0x4A4310`): a creature opens or closes an object's door (#1827).
const ACT_OBJ: u16 = 93;
/// The tags of an `ActObj` that close the door, the others opening it, as does none
/// (`0x4A4310`); the last of them counts.
const CLOSES: [u8; 4] = [0x06, 0x10, 0x27, 0x2C];
const OPENS: [u8; 4] = [0x07, 0x0F, 0x26, 0x2D];
/// The other records that wait their turn in a creature's queue under the anim mode, and the
/// rank of the `0x01` that names the creature: `Attack` the first (docs/re/netscript.md, "The
/// other kinds, the waits and the cinema").
const QUEUED_OTHERS: [(u16, usize); 1] = [(92, 0)];
/// `Wait` (`0x4A1F20`): the first `0x01` the creature whose queue takes it, the second the other,
/// one of the tags `0x58` to `0x5F` its condition (#1279).
const WAIT: u16 = 90;
/// The slots of a scripts' clip that end 0.2 s before the clip does (`0x545204`): the defending
/// row.
const DEFENDING: std::ops::RangeInclusive<u16> = 53..=65;

/// What a woman's type's `items.pak` name holds, as written (`0x955778`, `0x4FC490`).
const WOMAN: &str = "_FEM";

/// An NPC or an object to place.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct Placement {
    /// The serial of the creature made in play of it, a fill's or a summoned one's, by which a
    /// save names it (`names::Id::Born`, #3755): given as the creature is placed ([`named`]),
    /// none for the scripts' placements, which their index names.
    born: Option<u32>,
    /// The model, relative to the asset directory.
    model: String,
    /// Whether its type is a woman's (`minds::Mind::woman`).
    woman: bool,
    /// The global cell, its logical position.
    cell: (u32, u32),
    /// Where an object's model is drawn, if its record gives it ([`drawn_at`]): world units on
    /// the ground, the game's x and z.
    drawn: Option<Vec2>,
    /// Degrees.
    heading: f32,
    /// A `CreateObj`'s object rather than an NPC.
    object: bool,
    /// A horse's class, its `CreateNPC`'s tag `0x90` (`SetHorseClass:`, the byte `+0x400`), 0
    /// where none names it (#2299).
    horse_class: u8,
    /// A stable's horse's trader, by its script name: the `0x63` of its `CreateNPC` or of a
    /// `SetNPCState`, which stocks it (`stables::Stabled`, #2300).
    stock: Option<String>,
    /// A fill's black rider on its horse, which a sector's coming in seats again on the horse
    /// kept beside it (`0x493E2B`, #2288).
    rides: bool,
    /// The texture its item names (`+0x08`), relative to the asset directory.
    texture: Option<String>,
    /// The position's layer: a cell's third number, a `DefPos`'s fourth.
    layer: i32,
    /// The radius of the `DefPos` it was made at, in which it takes a cell of its own as it is
    /// made (`quests::Place::drawn`, #2442), 0 for a cell of its record's. A save keeps none:
    /// the placements it keeps are the fills' creatures', of no place, and the scripts' are made
    /// again of their records.
    #[serde(skip)]
    spread: i32,
    /// The sector whose `Enter` ran last as its `CreateNPC` of the operand [`BY_RULES`] ran, by
    /// whose `MakeGroup` rules it is armed once it is placed ([`arm_by_rules`], #3072). Not saved:
    /// a script's creature is made again of its record.
    #[serde(skip)]
    by_rules: Option<(u32, u32)>,
    /// The items of an NPC's `CreateNPC`, their types in its order (its `0x02`s after its type),
    /// and those it holds in its hands: their models, and the items themselves.
    items: Vec<u32>,
    equipment: Vec<Held>,
    holding: Vec<InHand>,
    /// The items it wears, each its type and its category (#2286): a `CreateNPC`'s that go into
    /// no hand, a fill's third of its `MakeGroup` rule, as the armour switch makes them.
    worn: Vec<(u32, u8)>,
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
    /// (`0x428130`; `ai`).
    door: bool,
    /// Whether it is a special, an object of category 15 (`cSpecial3D`: the signposts), which takes
    /// a label while Alt is held (`labels`, #1425).
    special: bool,
    /// The trigger that a door or a container names (`CreateObj`'s `0x29`), whose cells a door
    /// opens and closes (#372).
    trigger: Option<String>,
    /// A fire of the FX objects (`items.pak` category 12), its variant of the smoke class, its
    /// height in the models' units, the record's `0x7E` (`fx`, #1092), and whether its type's
    /// flag `0x800000` puts it in list 4 (#3286).
    fx: Option<(u8, f32, bool)>,
    /// An FX object that loops a sound of its own near the hero, an element stone or a stargate
    /// (`fx_loops`, #1170), which has no model of its item's: its sound and its type, a
    /// stargate's look its own (`stargates`), an element stone's its own (`element_stones`).
    fx_loop: Option<(u32, u32)>,
    /// An object of a big demon's summoning (`FX_BIG_DEMON_SUMMON`, 918; `summonings`, #2006):
    /// its variant, the record's `0x03`, and its height in the models' units, its `0x7E`; it has
    /// no model.
    summon: Option<(u8, f32)>,
    /// A piece of the scripts' Pong (1156 to 1158; `pong`, #2008) and its height in the models'
    /// units, its `0x7E`; it has no model.
    pong: Option<(crate::pong::Piece, f32)>,
    /// A magic bar or barrier of the FX objects (`FX_MAGICPRISON`, 854; `prisons`, #2063) or a
    /// bubble prison (`FX_BUBBLEPRISON`, 853; `bubble_prisons`, #2750): its type and its height in
    /// the models' units, the record's `0x7E`; it has no model.
    prison: Option<(u32, f32)>,
    /// A dwarves' magic of the FX objects (`FX_DWARFMAGIC`, 832 and 833; `dwarf_magics`), its
    /// variant, 1 or 2, and its height in the models' units, the record's `0x7E`; it has no model.
    dwarf_magic: Option<(u8, f32)>,
    /// An element of the forge's ritual of the FX objects (911 to 915; `forge_elements`, #2098):
    /// its variant, the type less 911, and its height in the models' units, the record's `0x7E`;
    /// it has no model of its item's.
    forge: Option<(u8, f32)>,
    /// An FX object of the scripts that a module of its own runs: the field of pills and ghosts
    /// (891, `pill_fields`) and the maker of the fences of lightning (894, `laser_fences`; #2916),
    /// a hell gate (1160, 1161, 1181, `hell_gates`; #2917), the Underworld's glyphs, their fire
    /// and the temple door's spikes (1174 to 1179, `underworld_glyphs`; #2918), Shaddar's slaves
    /// (921, `shaddar_slaves`; #2919), the sword's zhadum (1165, `zhadums`; #2920), the big
    /// demon's illusion (917, `demon_illusions`; #3059): its type and its height in the models'
    /// units, the record's `0x7E`; it has no model of its item's.
    scripted: Option<(u32, f32)>,
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
    /// Whether its `CreateNPC` carries `0x8D` ([`EMERGES`]): its creature plays its clip of the
    /// slot 154 once as it is made, a big demon turning hostile as it ends (#2461); a big demon
    /// turned keeps it no more ([`turn_emerged`]). A save keeps it, so that a creature saved in
    /// its clip plays it again as it comes back and turns as it ends (#3075; **hypothesis** for
    /// what the original's load makes of the clip).
    emerges: bool,
    /// Whether it is drawn under a liquid's surface while it stands in a liquid cell
    /// ([`models::Sinks`]): a `CreateObj`'s object of a type that `0x434070` passes, an item of a
    /// `Weapon.pak` record or one of the category `0x20`, that is no chest (`0x62AE90`; #3065).
    /// Not saved: a script's object is made again of its record.
    #[serde(skip)]
    sinks: bool,
    /// Whether its `CreateNPC` or a `SetNPCState` that names it marks it a vision ([`VISION`]),
    /// which it stays: its type is drawn and ends as the visions' types are ([`Vision`]).
    vision: bool,
    /// The text a fill's creature is called by, its rule's name ([`called_of`], [`Called`]).
    called: Option<u32>,
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
    /// fill.
    made: (bool, bool),
    /// Whether the start made it, its script or a quest it entered rather than a sector's
    /// function: its creature takes its level as the game starts ([`place`]).
    started: bool,
    /// Whether it is an effect of the start's of a type off the persistent list outside the 3 × 3
    /// sectors about its start, which the original makes at once in a sector that no `Enter` has
    /// marked and its tick ends at its first run (`0x5A6210`, [`ends_at_start`], #3049): never
    /// placed.
    #[serde(skip)]
    ended: bool,
    /// Whether it has a dialogue, a `CreateNPC` with one (the operand `0x09`; **hypothesis**: the
    /// one `0x483AFA` stamps; `ai::Mind`).
    dialogue: bool,
    /// Its trades, by its tags ([`TRADES`]; `opensacred_rules::minds::Mind::trader`), and whether
    /// its tags keep it out of its first update's flags ([`KEEPS`]).
    trades: u32,
    kept: bool,
    /// Its group, if it is in one.
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
    /// The floor its walk goes onto, of the building at its cell, 0 for the creature's own level
    /// as the record runs (`0x49E210`, #2481). Not saved: the placements a save keeps are the
    /// fills' creatures', which walk nowhere, and the scripts' are made again of their records.
    #[serde(skip)]
    walk_floor: usize,
    /// Whether the object maker alone made it (`0x5FBA40`), a crab or a fish of the waters, which
    /// no level's setter raises: its skills its two slots' at level 1, as its stats are made
    /// (`0x586C00`), and its most health the stats' 100 (seen in the original, #575).
    plain: bool,
    /// The clips an NPC plays.
    clips: ClipNames,
    /// An NPC's or an object's name in the scripts (`CreateNPC`'s or `CreateObj`'s `0x01`,
    /// lower-cased), by which their records name it (an object's a `DelNPC`'s, #2758), and the
    /// share of its most health that a `SetHP` gave it, in per cent (#1001).
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
    /// Whether such an item goes straight into the hero's backpack as its record runs, where it
    /// finds room ([`TO_PACK`], #3543).
    to_pack: bool,
    /// The function that the taking of such an item runs, its `CreateObj`'s [`ON_TAKE`], as the
    /// pick-up runs it whichever way the item goes (`0x558080`, #2891).
    taking: Option<String>,
    /// An object's item only raised to its level, not made anew (`CreateObj`'s [`RAISED`],
    /// the scripts' own uniques; #1125).
    raised: bool,
    /// A container's (an object of [`CHEST`]): its `Take:` function, its `CreateObj`'s
    /// [`ON_TAKE`], which fills it as it opens (`containers`, #1380).
    container: Option<Option<String>>,
    /// An object's item's `Use:` function, its `CreateObj`'s [`ON_USE`], which the right button
    /// runs on it in the backpack (#1272).
    uses: Option<String>,
    /// An NPC's script's spell, its `CreateNPC`'s last [`ART`] ([`script_spell`], `+0x2B6`), which
    /// a magician's chooser takes first (`0x4D31C0`, #1869); 0 for none.
    spell: u8,
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
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct ClipNames {
    stand: Option<String>,
    walk: Option<String>,
    run: Option<String>,
    /// A flyer's clip aloft, the slot 179 (`*_FLY_LOOP`, `0x546ABF`, #2250).
    fly: Option<String>,
    stance: Option<String>,
    talk: Option<String>,
    /// The jugglers' clips of the slots 180 to 183 (`*_FUN_IDLE_*`), by which they play their
    /// instruments.
    fun: [Option<String>; 4],
    /// Its attack's clips (`motions::Slots::attacks`), and whether they are a stand-in for its
    /// class's, which `MOTIONS.PAK`'s record 0 times ([`LACKING`], #3314).
    attacks: Vec<String>,
    #[serde(default)]
    stand_in: bool,
    /// Its dying clips by their slots: 173 and 174 (`*_DYING_A`, `*_DYING_B`) and the flat
    /// falls 228 to 233, of which [`die`] rolls one at each death
    /// (`opensacred_rules::undead::dying_slot`, #3400); a rider's 168, else 173.
    dying: Vec<(u16, String)>,
    /// Whether its model has no clip in the slot 173, by which its body vanishes at its first
    /// tick of lying (`0x426250(type, 0xAD)`, `0x4117B0`; `bodies`, #2514).
    unfallen: bool,
    /// The fidgets' slots 180 to 192 that its model has, bit `i` the slot `180 + i`
    /// (`minds::Mind::fun`), and its attack's first slot, which a fidget plays where it takes
    /// none of them (`0x5467A0(4)`, #1364).
    fun_slots: u16,
    attack_slot: u16,
    /// Its defending slot, where its model has one (`motions::Slots::defend_slot`): its parry's
    /// (#2487).
    defend: Option<u16>,
}

/// The small spider, whose every action plays its walk, the slot 27 (`0x5467A0`, #2913).
const SMALL_SPIDER: u32 = 71;
const SCUTTLE: u16 = 27;

impl ClipNames {
    /// These clips for the creature of the type `creature`: the small spider's every action on
    /// its walk's clip, its slot 27, whatever it does, so that its legs always scuttle (`0x5467A0`,
    /// before its table; docs/re/motions.md, "The other cases"); any other's as they are. Its
    /// walk's speed by its gait and its strike's hits by the walk's clip (**hypothesis**: neither
    /// is read).
    fn of_creature(self, creature: u32) -> Self {
        let Some(walk) = self.walk.clone().filter(|_| creature == SMALL_SPIDER) else {
            return self;
        };
        Self {
            stand: Some(walk.clone()),
            run: Some(walk.clone()),
            stance: Some(walk.clone()),
            talk: Some(walk.clone()),
            fun: [None, None, None, None],
            fun_slots: 0,
            attacks: vec![walk.clone()],
            stand_in: false,
            dying: vec![(opensacred_rules::undead::DYING_A, walk.clone())],
            attack_slot: SCUTTLE,
            defend: Some(SCUTTLE),
            walk: Some(walk),
            ..self
        }
    }

    /// The clips of a rider of a model of `slots` holding what is of `class`, on its horse
    /// (`0x5467A0` on horseback, #2288): standing its slot 169, else 166, moving 166, its strike
    /// the rider's of its weapon's class (`motions::Slots::riding_attack`), its death 168, else
    /// 173; no stance, fidgets or juggling.
    fn riding(slots: &motions::Slots, class: u8) -> Self {
        let named = |slot| slots.slot(slot).map(str::to_owned);
        let ride = named(RIDE);
        Self {
            stand: named(RIDE_IDLE).or_else(|| ride.clone()),
            walk: ride.clone(),
            run: ride,
            stance: None,
            attacks: slots
                .riding_attack(class)
                .map(|name| vec![name.to_owned()])
                .unwrap_or_default(),
            stand_in: false,
            dying: named(RIDING_DEATH)
                .map(|name| (RIDING_DEATH, name))
                .or_else(|| {
                    named(opensacred_rules::undead::DYING_A)
                        .map(|name| (opensacred_rules::undead::DYING_A, name))
                })
                .into_iter()
                .collect(),
            fun: [None, None, None, None],
            fun_slots: 0,
            ..Self::of(slots, class)
        }
    }

    /// The clips of a model of `slots` that holds what is of `class` (`motions::class`).
    fn of(slots: &motions::Slots, class: u8) -> Self {
        let clip = |action| slots.clip(action, class).map(|(name, _)| name.to_owned());
        let attacks = slots.attacks(class);
        Self {
            stand: clip(motions::Action::Stand),
            walk: clip(motions::Action::Walk),
            run: clip(motions::Action::Run),
            fly: slots.slot(FLY).map(str::to_owned),
            stance: slots.stance(class).map(str::to_owned),
            attacks: attacks.clips,
            stand_in: attacks.stand_in,
            dying: dying_names(slots),
            unfallen: slots.slot(173).is_none(),
            talk: slots.talk().map(str::to_owned),
            fun: [180, 181, 182, 183].map(|slot| slots.slot(slot).map(str::to_owned)),
            fun_slots: (0..minds::FUN_COUNT)
                .filter(|&i| slots.slot(minds::FUN_SLOTS + i).is_some())
                .fold(0, |mask, i| mask | (1 << i)),
            attack_slot: motions::Slots::attack_slot(class),
            defend: slots.defend_slot(class),
        }
    }
}

/// The dying clips of a model of `slots` by their slots, the plain falls 173 and 174 and the
/// flat ones 228 to 233 that it has (`0x53EF90`–`0x53F31D`, #3400).
pub(crate) fn dying_names(slots: &motions::Slots) -> Vec<(u16, String)> {
    use opensacred_rules::undead::{DYING_A, DYING_B, DYING_FLAT, DYING_FLAT_OTHERS};
    [DYING_A, DYING_B, DYING_FLAT]
        .into_iter()
        .chain(DYING_FLAT_OTHERS)
        .filter_map(|slot| slots.slot(slot).map(|name| (slot, name.to_owned())))
        .collect()
}

/// The index among `dying`, the dying clips by their slots, of the one that a death of the type
/// `creature` takes: rolled ([`opensacred_rules::undead::dying_slot`]), the first where it rides,
/// 0 where none is the slot rolled.
pub(crate) fn dying_choice(
    creature: u32,
    dying: &[u16],
    rides: bool,
    dice: &mut crate::spawning::Rng,
) -> u32 {
    if rides {
        return 0;
    }
    let slot = opensacred_rules::undead::dying_slot(creature, |slot| dying.contains(&slot), dice);
    dying
        .iter()
        .position(|&had| had == slot)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0)
}

/// The dice of the dying clips' rolls (`rand()` at `0x53EF90`).
#[derive(Resource)]
pub(crate) struct DyingDice(pub(crate) crate::spawning::Rng);

impl Default for DyingDice {
    fn default() -> Self {
        Self(crate::seed::dice(0x228))
    }
}

/// The rider's slots on horseback: the ride, the idle and the death (docs/re/horses.md).
const RIDE: u16 = 166;
const RIDE_IDLE: u16 = 169;
const RIDING_DEATH: u16 = 168;

/// A fill's black rider's horse and the rider that it brings, the undead's side that both take
/// and the rider's weapons by a coin, the magic short bow and the tribal axe (`FillSector` at
/// `0x47E863`–`0x47EFA5`, #2288).
const BLACK_HORSE: u32 = 261;
const BLACK_RIDER: u32 = 256;
const UNDEAD_SIDE: u8 = 5;
const BLACK_RIDER_WEAPONS: [u32; 2] = [1811, 1778];

/// The instruments a juggler takes (`0x4F29E2`): the flute, the bell, the harp and the horn, their
/// `items.pak` slots and the slots of the clips that play them.
const INSTRUMENTS: [(u32, u16); 4] = [(5108, 183), (5109, 180), (5110, 182), (5111, 181)];
/// The first of the jugglers' clips' slots; the bell's, which one without an instrument plays.
const FUN: u16 = 180;
/// The slot of a flyer's clip aloft (`0x546ABF`, `0x5967C0`): the crows' `CROW_FLY_LOOP`, the
/// bats' `BATX_FLY_LOOP`.
const FLY: u16 = 179;

/// `FX_OPAQUE.TGA`, the texture record 203: 16 × 16 texels of nothing, which the items that the
/// original draws by their effects name (the Seraphim's 3809, its glow): such an item takes it
/// where its effect is drawn (#1834), its effect showing whole, else its model's own material.
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
/// The category of the waters' crab and fish (163 to 166), which `0x434070` passes as it does an
/// item's type: an object of it is drawn under a liquid's surface ([`models::Sinks`]).
const SINKING: u8 = 0x20;
/// The category of the doors (`items.pak` 5650 to 5766, `0x428130`).
const DOOR: u8 = 10;
/// The category of the specials (`cSpecial3D`, `0x451300`): the signposts 4993 to 4999.
const SPECIAL: u8 = 15;
/// The FX objects' category (`cObjectFX`, docs/re/effects.md).
const FX: u8 = 12;
/// A `CreateObj`'s height of an FX object, in the models' units (`0x4861BF`).
const FX_HEIGHT: u8 = 0x7E;
/// The FX type of a big demon's summoning (`FX_BIG_DEMON_SUMMON`; `summonings`).
const DEMON_SUMMON: u32 = 918;
/// The FX type of the scripts' magic bars and barriers (`FX_MAGICPRISON`; `prisons`).
const MAGIC_PRISON: u32 = 854;
/// The FX type of the scripts' bubble prisons (`FX_BUBBLEPRISON`; `bubble_prisons`, #2750).
const BUBBLE_PRISON: u32 = 853;
/// The dwarves' magics, `CreateObj`'s variants 1 and 2 (`0x4898B2`, `0x489920`).
const DWARF_MAGICS: [u32; 2] = [832, 833];
/// `CreateObj`'s mark of a quest's item that it lays in the world, an object of `+0x14` bit
/// `0x800` (`0x485C10`), and the function that its taking runs, kept behind `Take:` (docs/re/
/// netscript.md, "The log's quest items"; #1256).
const QUEST_ITEM: u8 = 0x8E;
const ON_TAKE: u8 = 0x41;
/// The `CreateObj`'s tag of an item's use's function (`Use:`, `0x486263`).
const ON_USE: u8 = 0x83;
/// The `CreateObj`'s tag that gives its item straight into the local hero's backpack, marked
/// until it is moved (`0x48862D` to `0x48873E`; docs/re/inventory.md, "Finding, testing and
/// placing"; #3543).
const TO_PACK: u8 = 0x3F;
/// The keys' category (`0x428170`), whose items such a `CreateObj` puts into the keys' list
/// rather than the backpack, their objects destroyed (`0x4886F2`; docs/re/containers.md, "The
/// triggers' keys").
const KEY: u8 = 14;
/// A quest's item's quality key as a drop: a plain item's, made by no item maker.
const QUEST_KEY: u8 = 16;
/// `CreateObj`'s mark of an item only raised to its level (`0x485E70`, the flag that `0x487D1D`
/// tests): the scripts' own uniques 7911 to 7930 and a few others.
const RAISED: u8 = 0x9E;
/// The bit of an `items.pak` record's byte `+0x02` of a type that nobody picks up, its first
/// word's bit 22 (`0x427920`): the signposts 4993 to 4999 and `windstrauch.grn` (#1476).
const UNTAKEABLE: u8 = 0x40;

/// Whether a creature may pick up an item of the type `item` (`0x427920`; docs/re/items.md,
/// "The walk"): of the categories 5 to 9, 13 to 25, 28, 29 and 33, and without [`UNTAKEABLE`].
fn takeable(item: &Item) -> bool {
    matches!(category(item), 5..=9 | 13..=25 | 28 | 29 | 33) && item.subclass & UNTAKEABLE == 0
}

/// An item that a `CreateObj` of an item's type without [`QUEST_ITEM`] lays in the world, which the
/// hero picks up as any lying item (seen in the original, #1296): its placement, which is not
/// placed again once taken ([`Npcs::taken`]), its `Use:` function ([`ON_USE`], #1272), and the
/// function its taking runs ([`ON_TAKE`], `Take:`; seen in the original, #2891).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct ScriptItem {
    pub index: usize,
    pub uses: Option<String>,
    pub take: Option<String>,
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
/// special arrows' and the bow arts' (`opensacred_rules::arrows::projectile`).
const ARROWS: [u32; 8] = [
    arrows::ARROW,
    arrows::MULTI_ARROW,
    arrows::HARD_ARROW,
    arrows::ZEAL_ARROW,
    3074,
    3075,
    3077,
    3078,
];

/// A hand of an NPC's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Hand {
    Left,
    Right,
}

impl Hand {
    /// The hand of the equipment's slot `slot`: 12 the left, 13 the right (`0x556F50`).
    fn of_slot(slot: u8) -> Option<Self> {
        match slot {
            equipment::LEFT_HAND => Some(Self::Left),
            equipment::RIGHT_HAND => Some(Self::Right),
            _ => None,
        }
    }
}

/// Where an item's model goes on its holder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Held {
    /// The item's type, by which its model draws its own glows (`item_glows`); 0 for none.
    pub(crate) item: u32,
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
    /// The item of the type `item`, the model `model` and the texture `texture` worn in the slot
    /// `slot` of the hero's equipment (#1307): on its back in the slot 17, the wings', else on its
    /// skeleton (docs/re/equipment.md, "The slots").
    pub fn worn(slot: u8, item: u32, model: String, texture: Option<String>) -> Self {
        Self {
            item,
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

    /// The item that the model of `piece` draws, worn in no slot of the hero's equipment.
    fn of_piece(piece: &Piece) -> Self {
        Self {
            item: piece.item,
            model: piece.model.clone(),
            texture: piece.texture.clone(),
            hand: piece.hand,
            mount: piece.mount,
            slot: None,
        }
    }

    /// What its model on its holder is made of ([`Piece`]).
    fn piece(&self) -> Piece {
        Piece {
            item: self.item,
            model: self.model.clone(),
            texture: self.texture.clone(),
            hand: self.hand,
            mount: self.mount,
        }
    }
}

/// What the model of an item on its holder is made of (`equip`): its model, its texture, its
/// hand and where it goes, by which a hero whose form keeps its model keeps the pieces it still
/// shows (`hero::spawn`, #1600).
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    item: u32,
    model: String,
    texture: Option<String>,
    hand: Hand,
    mount: Mount,
}

impl Piece {
    /// Its item's type, 0 for a model of no type (an instrument's).
    pub fn item(&self) -> u32 {
        self.item
    }

    /// Its model, relative to the asset directory.
    pub fn model(&self) -> &str {
        &self.model
    }
}

/// An item in an NPC's hand, whose values it fights with: its type, its category, its weapon's
/// kind, and the hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// Whether a creature strikes with two weapons, its items in their hands `holding` of the class
/// `class` (`0x4EA3F0`): the class 8, or an item in each hand, the left no shield.
fn two_weapons_of(holding: &[InHand], class: u8) -> bool {
    let holds = |hand, shield: Option<bool>| {
        holding
            .iter()
            .any(|held| held.hand == hand && shield.is_none_or(|s| (held.category == SHIELD) == s))
    };
    class == opensacred_rules::fight::TWO_WEAPONS_CLASS
        || (holds(Hand::Right, None) && holds(Hand::Left, Some(false)))
}

/// Whether an `EquipNPC` takes the item `item` of the category `category` and the kind `kind`
/// out of a hand before it equips another (`0x5CECD0`): a weapon of the kinds 2, 3, 4, 7 or 12,
/// or of the types 4067 to 4072.
fn taken_out(item: u32, category: u8, kind: Option<u8>) -> bool {
    category == WEAPON
        && (kind.is_some_and(|kind| matches!(kind, 2 | 3 | 4 | 7 | 12))
            || (4067..=4072).contains(&item))
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
    /// Whether its `items.pak` name holds `_FEM` as written (`minds::Mind::woman`).
    woman: bool,
}

/// What a fill's creature of type `creature`, of `kind`, whose model has the clips `slots`, is
/// placed as: its model standing with its hands empty, its shadow, and its flags word, to which a
/// hostile entry adds bit 0.
fn template(creature: u32, kind: &Kind, slots: &motions::Slots) -> Placement {
    Placement {
        born: None,
        model: kind.model.clone(),
        horse_class: 0,
        stock: None,
        rides: false,
        cell: (0, 0),
        drawn: None,
        home: None,
        walk: None,
        walk_floor: 0,
        speed: kind.speed,
        class: kind.class,
        base: kind.base,
        held: held_class(&[]),
        ranged: false,
        size: kind.size,
        made: (true, false),
        started: false,
        ended: false,
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
        special: false,
        fx: None,
        fx_loop: None,
        summon: None,
        pong: None,
        prison: None,
        dwarf_magic: None,
        forge: None,
        scripted: None,
        trigger: None,
        creature: (creature, kind.flags, 0),
        side: kind.class,
        rises: false,
        listed: false,
        hostility: None,
        emerges: false,
        sinks: false,
        vision: false,
        called: None,
        texture: kind.texture.clone(),
        woman: kind.woman,
        layer: 0,
        spread: 0,
        by_rules: None,
        items: Vec::new(),
        equipment: Vec::new(),
        holding: Vec::new(),
        worn: Vec::new(),
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
        to_pack: false,
        taking: None,
        raised: false,
        container: None,
        uses: None,
        spell: 0,
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
    /// What the scripts' records are placed with.
    makers: Makers,
    /// The horse data's rows, each its class and its seven words (`Balance.bin`'s tail table 4,
    /// #2299).
    horses: Vec<(u32, [u32; 7])>,
    /// The places that the scripts' `DefPos`s name, and the placements that the play's records
    /// made whose sectors were in the world (#1014).
    places: Places,
    pending: Vec<usize>,
    /// The placements that the play's records made, which live wherever they stand from then on
    /// ([`Self::living_groups`]).
    played: HashSet<usize>,
    /// The play's records of the world in the batches that [`script_records`] placed, in order,
    /// which a save keeps and its load places again ([`Self::resume`], #1958).
    records: Vec<Vec<Record>>,
    /// The placements of the quests' items and of the scripts' other lying items that the hero
    /// took, which their sectors do not place again ([`QuestItem`], #1256; [`ScriptItem`], #1296).
    taken: HashSet<usize>,
    /// The placements of the objects that a play's `DelNPC` removed by their names, which their
    /// sectors do not place again (#2758); a save's records remove them again as it loads. Shaddar's
    /// slaves too, once placed: they end with their sector, never made again (#2919).
    removed: HashSet<usize>,
    /// The level that each placement's item lying was made at the first time it was placed,
    /// which its sector's later comings make it at again (`CreateObj`'s, #1125).
    laid: HashMap<usize, u32>,
    /// The placements of the containers that the hero emptied, which their sectors place again
    /// emptied ([`crate::containers::Container`], #1380).
    pub(crate) emptied: HashSet<usize>,
    /// The dialogues' table by name, as the start's records fill it, its first entry twice
    /// (`0x476A05`), which the play's `SetGroupState`s look their dialogues up in (#1021), and
    /// each entry's icon, as the scripts' `SetIcon`s leave it (#1039).
    dialogues: Vec<String>,
    icons: Vec<i32>,
    /// The dialogue of each creature type that the start's `SetRgnDialog`s give, all of the
    /// region 0 (`0x465280`, #1028).
    region_talks: HashMap<u32, String>,
    /// The sectors' values, which their creatures' levels take, and their fills ([`sectors()`]),
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
    respawns: Respawns,
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
    /// The floats of every clip from its hit points on ([`hit_points`]), of which a creature's
    /// art's clip takes its hit points (`creature_arts`, #2564).
    points: HashMap<String, Vec<f32>>,
    spawned: HashSet<(u32, u32)>,
    /// The sector whose `Enter` ran last, whose `MakeGroup` rules the rules' vector holds
    /// (`0x4939F0` empties it before it runs a sector's `Enter`), by which a `CreateNPC` of the
    /// operand `54` arms its creature ([`BY_RULES`], #3072).
    last_enter: Option<(u32, u32)>,
    /// What became of the creatures that went with their sectors ([`Kept`]).
    kept: Kept,
    /// A loaded save's queues of the kept creatures, by their placements, which each takes again
    /// as its sector brings it back ([`Creatures`], #2890).
    queues: HashMap<usize, SavedQueue>,
    /// A loaded save's `Enter` creatures of the sectors that were in the world, by their
    /// placements: where each stood, or none where it was killed, which each takes at its
    /// sector's first coming after the load alone ([`entered_again`], #3761).
    entered: HashMap<usize, Option<Stood>>,
    /// The sectors come in with no creatures kept whose waters' crabs and fish wait for the
    /// sectors about them ([`swimmers`]).
    waters: Vec<(u32, u32)>,
    /// The cells that the creatures made at a `DefPos` of a radius took in this frame's placing,
    /// which the next made there leaves to them ([`spread_cell`], #2442).
    drawn: HashSet<(i32, i32)>,
    /// The level of the hero's riding (the skill 22 with its items' bonus, `0x582450`), an
    /// eighth of which a horse that `CreateNPC` makes takes ([`Self::scripted_level`]),
    /// [`hero_riding`] keeping it.
    riding: u32,
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

impl Placed {
    /// The index of its kept placement, none for a fill's creature.
    pub(crate) fn kept(&self) -> Option<usize> {
        match self.source {
            Source::Kept(index) => Some(index),
            Source::Filled(_) => None,
        }
    }
}

/// A special of the type, an object of category 15 (`cSpecial3D`: the signposts), which takes a
/// label while Alt is held (`labels`, #1425).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Special(pub u32);

/// The placement a creature was placed of, by its index, which a loaded save's start makes again
/// at the same index (#1958): by it the merchants' stock knows its merchant again after a load
/// (`merchant`, #1959), where its entity is another.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacementIndex(pub usize);

/// The name of a script's object, its `CreateObj`'s `0x01` in lower case, by which a `DelNPC`
/// destroys it (`0x497F80`, #2758); a creature's is its [`ScriptName`].
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct ObjectName(pub String);

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

/// Where a creature that went with its sector stood, on which floor, which way it faced, its
/// health, and whether a bonus against the undead marked it (`undead::Marked`, #3385): none for a
/// body that its dialogue holds (#2457), which lies there.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Stood {
    cell: (u32, u32),
    layer: i32,
    /// The heading it faced, in a placement's degrees ([`degrees`]), as the original's creature
    /// keeps its direction `+0x70` in its record (`0x44FC50`, #3744).
    heading: f32,
    health: fight::Health,
    marked: bool,
}

impl Stood {
    /// Whether it is a body that lies there ([`stood_of`]).
    fn lies(&self) -> bool {
        self.health.current == 0
    }
}

/// What became of the placed creatures ([`Placed`]) that went with their sectors.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
// Named apart from another saved type of its name, which the saves' layout test traces by
// name (#2306); postcard writes no names.
#[serde(rename = "KeptCreatures")]
struct Kept {
    /// The kept placements that went, by index: where each stood or its held body lies, or
    /// `None` once killed.
    went: HashMap<usize, Option<Stood>>,
    /// The kept placements spawned, living somewhere: not placed again while they are.
    out: HashSet<usize>,
    /// The living creatures of each sector's fills that went with it.
    filled: HashMap<(u32, u32), Vec<(Placement, Stood)>>,
    /// The sectors that kept a time, keeping a fill's living creature as they went (`+0x59C`).
    times: HashSet<(u32, u32)>,
    /// The region whose sector filled last (`0x94E080`).
    last: Option<u16>,
    /// The next serial of a creature made in play (`names::Id::Born`, #3755).
    born: u32,
    /// The world's time of each sector's respawn's gate's last pass, by [`Self::clock`] (#3786).
    passed: HashMap<(u32, u32), f64>,
    /// The world's time since the game began, in seconds, which a pause stands and a save keeps:
    /// the respawn's gate's clock, in place of the original's real clock.
    clock: f64,
    /// The sectors that were in the world at a save, whose first coming after its load puts back
    /// what they kept whatever the gate and the region, as their creatures stood in the world
    /// (#3761), as the original's sectors of the bit `0x800` run no `Enter` at a load.
    standing: HashSet<(u32, u32)>,
}

/// The respawn's bounds (`Respawn`, `DungeonRespawn` of the balance): the seconds a sector waits
/// after its gate last passed before it may fill anew, a dungeon's longer.
#[derive(Clone, Copy, Debug)]
struct Respawns {
    open: f32,
    dungeon: f32,
}

/// The balance's respawn's bounds, 300 and 1,000 s where `tables/balance.ron` lacks them.
fn respawns(balance: Option<&opensacred_data::tables::Balance>) -> Respawns {
    let bound = |name, default| balance.and_then(|b| b.f32(name)).unwrap_or(default);
    Respawns {
        open: bound("Respawn", 300.0),
        dungeon: bound("DungeonRespawn", 1_000.0),
    }
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
    /// A new serial of a creature made in play.
    fn mint(&mut self) -> u32 {
        let serial = self.born;
        self.born = self.born.wrapping_add(1);
        serial
    }

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
    /// spawning.md): within `bound` seconds of its respawn's gate's last pass it fills nothing,
    /// the creatures it kept coming back and all else as it is (docs/decisions/spawning.md,
    /// "Respawn's gate as meant"); else the gate passes, and where it kept a time and its region
    /// filled last, the creatures it kept come back, none where it kept none, and it fills
    /// nothing; else it fills anew, dropping what it kept, and a region other than the last drops
    /// what every sector of another region kept, its time and its gate's pass. A sector of no
    /// fills puts back what it kept (`0x4939F0`).
    fn enter(
        &mut self,
        at: (u32, u32),
        filling: Option<u16>,
        bound: f64,
        region_of: impl Fn((u32, u32)) -> u16,
    ) -> Entered {
        let back = self.filled.remove(&at).unwrap_or_default();
        if self.standing.remove(&at) {
            return Entered { back, fill: false };
        }
        let Some(region) = filling else {
            return Entered { back, fill: false };
        };
        if self
            .passed
            .get(&at)
            .is_some_and(|&pass| self.clock < pass + bound)
        {
            bevy::log::debug!("npcs: {at:?} fills no sooner than {bound} s after its last");
            return Entered { back, fill: false };
        }
        self.passed.insert(at, self.clock);
        if self.times.contains(&at) && self.last == Some(region) {
            return Entered { back, fill: false };
        }
        if self.last != Some(region) {
            bevy::log::debug!("npcs: the region {region} fills after {:?}", self.last);
            self.filled.retain(|&sector, _| region_of(sector) == region);
            self.times.retain(|&sector| region_of(sector) == region);
            self.passed.retain(|&sector, _| region_of(sector) == region);
            self.last = Some(region);
        }
        Entered {
            back: Vec::new(),
            fill: true,
        }
    }

    /// Where the kept placement `index` of the cell `cell` lives away from the world
    /// (`Npcs::lives_away`): nowhere while it is in the world, where it stood once it went
    /// living, nowhere once killed or lying dead, and at its own cell before it was ever placed
    /// where its maker `made` it already.
    fn away(&self, index: usize, cell: (u32, u32), made: bool) -> Option<(u32, u32)> {
        if self.out.contains(&index) {
            return None;
        }
        match self.went.get(&index) {
            Some(stood) => stood.filter(|stood| !stood.lies()).map(|stood| stood.cell),
            None => made.then_some(cell),
        }
    }

    /// Where the body of the kept placement `index` lies away from the world, its dialogue holding
    /// it (#2457).
    fn lies(&self, index: usize) -> Option<(u32, u32)> {
        if self.out.contains(&index) {
            return None;
        }
        self.went
            .get(&index)
            .copied()
            .flatten()
            .filter(Stood::lies)
            .map(|stood| stood.cell)
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
                // Kept with its sector while it lives, which keeps a time then (`0x4954C0`).
                if let Some(stood) = stood.filter(|stood| !stood.lies()) {
                    self.filled
                        .entry(placed.at)
                        .or_default()
                        .push(((**placement).clone(), stood));
                    self.times.insert(placed.at);
                }
            }
        }
    }

    /// What it keeps once the creatures `standing` went, each where it stood or killed, as their
    /// sectors keep them going ([`Self::gone`]): a save keeps the sectors in the world so (#1959).
    fn with_gone<'a>(
        &self,
        standing: impl IntoIterator<Item = (&'a Placed, Option<Stood>)>,
    ) -> Self {
        let mut kept = self.clone();
        for (placed, stood) in standing {
            kept.gone(placed, stood);
        }
        kept
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
    /// The level it stands on: the ground, or a building's floor, which a walk to it goes onto
    /// (`containers::pick`, #1978).
    pub level: Level,
}

/// The texture a placed model takes in place of its own: its item's (`items.pak +0x08`).
#[derive(Component)]
pub struct Retexture(Handle<Image>);

impl Retexture {
    /// The texture `texture`, relative to the asset directory, repeated ([`load_texture`]).
    pub fn load(assets: &AssetServer, texture: &str) -> Self {
        Self(load_texture(assets, texture))
    }

    /// The texture.
    pub fn image(&self) -> &Handle<Image> {
        &self.0
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
    /// How its strike's hits fall by what it holds ([`Hits`]).
    hits: Hits,
    /// The factor of its walk and run speeds and of their clips' rate, its pace's
    /// (`opensacred_rules::fight::Combat::pace_factor`, #599).
    pace: f32,
}

/// How a creature's strike's hits fall by what it holds (`0x4EA1F0`, `0x4EA3F0`): the class of
/// what it holds, by which a clip of one hit point hits twice with two weapons, and its hits' share
/// of a strike, halved with two weapons (`opensacred_rules::fight::Holds::share`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Hits {
    class: u8,
    share: f32,
}

impl Npc {
    /// Its hits' share of a strike: a half with two weapons ([`Hits`]).
    pub(crate) fn share(&self) -> f32 {
        self.hits.share
    }

    /// Its attack's first slot, which one of its fidgets plays ([`Self::fidget_slot`]).
    pub(crate) fn attack_slot(&self) -> u16 {
        self.names.attack_slot
    }

    /// The slot of the clip that its fidget `fidget` plays (`0x54EA30`, #1364): a fun clip's own,
    /// or its attack's first.
    pub(crate) fn fidget_slot(&self, fidget: minds::Fidget) -> u16 {
        match fidget {
            minds::Fidget::Fun(slot) => slot,
            minds::Fidget::Attack => self.names.attack_slot,
        }
    }

    /// Whether its model has a dying clip in the slot 173 (`0x426250(type, 0xAD)`, `0x4117B0`),
    /// without which its body vanishes at its first tick of lying (`bodies`, #2514).
    pub(crate) fn falls(&self) -> bool {
        !self.names.unfallen
    }

    /// The factor of its walk and run speeds and of their clips' rate ([`repace`]).
    pub fn pace(&self) -> f32 {
        self.pace
    }

    /// The slot of its parry, where its model has its defending clip (#2487).
    pub(crate) fn defend_slot(&self) -> Option<u16> {
        self.names.defend
    }

    /// Whether its dying clip `choice`, as [`die`] plays it, has played to its end: at once for
    /// a model of none, never before its clips are made.
    pub(crate) fn died(&self, choice: u32, players: &Query<&AnimationPlayer>) -> bool {
        let (Some(graph), Some(player)) = (&self.graph, self.player) else {
            return false;
        };
        let Some(&(_, node)) = graph
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

    /// The clip that a horse carried under its rider plays now by its gait `steered` (`riding`,
    /// #2299; its walk, or its run where `Some(true)`), as [`Self::walking`] tells a walk's;
    /// none where it stands (`steps`, #1180).
    pub(crate) fn steered(
        &self,
        steered: &Steered,
    ) -> Option<(Entity, AnimationNodeIndex, &str, bool)> {
        let graph = self.graph.as_ref()?;
        let runs = steered.0? && graph.run.is_some();
        let (node, name) = if runs {
            (graph.run?, self.names.run.as_deref()?)
        } else {
            (graph.walk?, self.names.walk.as_deref()?)
        };
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
    fly: Option<AnimationNodeIndex>,
    stance: Option<AnimationNodeIndex>,
    /// A second node of the idle clip and of the stance's, into which a shot's end blends the
    /// standing clip in play to start it anew, a clip blending into no node of its own
    /// ([`Self::again`], #3562).
    idle_again: AnimationNodeIndex,
    stance_again: Option<AnimationNodeIndex>,
    talk: Option<AnimationNodeIndex>,
    fun: [Option<AnimationNodeIndex>; 4],
    /// The attack's clips, each with its clip and the fractions at which it hits.
    attacks: Vec<(AnimationNodeIndex, Handle<AnimationClip>, [f32; 2])>,
    /// The dying clips by their slots ([`ClipNames::dying`]).
    dying: Vec<(u16, AnimationNodeIndex)>,
    /// The clips of the slots that the scripts' `PlayAnim`s played, added to its graph as they
    /// come (#1277), each with its clip.
    scripted: HashMap<u16, (AnimationNodeIndex, Handle<AnimationClip>)>,
    /// Its hit clip's node laid over the clip in play, added the first time it plays, apart
    /// from the scripts' clips ([`hit_layer`], #3637), with its clip.
    layered: Option<(AnimationNodeIndex, Handle<AnimationClip>)>,
}

impl Clips {
    /// The other node of the standing clip `node`, its idle or its stance, none for another clip.
    fn again(&self, node: AnimationNodeIndex) -> Option<AnimationNodeIndex> {
        if node == self.idle {
            Some(self.idle_again)
        } else if node == self.idle_again {
            Some(self.idle)
        } else if Some(node) == self.stance {
            self.stance_again
        } else if Some(node) == self.stance_again {
            self.stance
        } else {
            None
        }
    }

    /// Whether `node` plays the clip of `wanted`, its own node or the other of a standing clip.
    fn same(&self, node: AnimationNodeIndex, wanted: AnimationNodeIndex) -> bool {
        node == wanted || self.again(node) == Some(wanted)
    }

    /// The clip of a walk: its flight's where it flies and has one (`0x546ABF`), its run clip
    /// where it runs and has one, else its walk clip.
    fn moving(&self, walk: &Walk) -> Option<AnimationNodeIndex> {
        if !walk.moved {
            None
        } else if walk.flies {
            self.fly.or(self.run).or(self.walk)
        } else if walk.runs {
            self.run.or(self.walk)
        } else {
            self.walk
        }
    }
}

/// An NPC walking where a script sent it (`NPC_Goto`, `0x49E210`, which orders the creature to
/// walk there) or where its mind orders it (`ai`, `followers`, `flight`): the point, the level it
/// walks on, and its speed in world units a second, before the game's speed.
#[derive(Component)]
pub struct Walk {
    target: Vec3,
    level: Level,
    speed: f32,
    /// Whether it runs, playing its run clip, and whether it flies (`+0x1F4` bit `0x200000`,
    /// #2250), playing its flight's: aloft no creature stops it, and it passes over the low
    /// cells of the nibble 2 and the closed doors (`0x4F7E10`, `0x4EBCA0`).
    runs: bool,
    flies: bool,
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
    /// The leg it heads for and its course there, found as it set out on the leg and again as it
    /// moves (`paths::Planned`, `0x4F7710`, `0x4F4770`, #2175).
    course: Option<paths::Planned<Floored>>,
    /// The level it walks onto, where it is given (`0x4F7710`'s position's floor): a follower's
    /// step off's (#1013); else any its way reaches the target's cell on.
    onto: Option<Level>,
}

/// A walk as a save keeps it (#3758; the original's creature's moving action, `+0x150` to
/// `+0x1A3`, docs/re/saves.md, "A creature's record"): where it goes, on what level, how, and how
/// near it stops; its course is found anew from where it stands, as its search runs again as it
/// moves (`paths::Planned`).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedWalk {
    target: Vec3,
    level: Level,
    speed: f32,
    runs: bool,
    flies: bool,
    climbed: bool,
    moved: bool,
    balked: f32,
    within: f32,
    comes_on: bool,
    idle: bool,
    onto: Option<Level>,
}

impl MapEntities for SavedWalk {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, _: &mut E) {}
}

impl crate::doings::Saves for Walk {
    type Saved = SavedWalk;

    fn saved(&self, _: &crate::doings::Moment) -> Option<SavedWalk> {
        Some(SavedWalk {
            target: self.target,
            level: self.level,
            speed: self.speed,
            runs: self.runs,
            flies: self.flies,
            climbed: self.climbed,
            moved: self.moved,
            balked: self.balked,
            within: self.within,
            comes_on: self.comes_on,
            idle: self.idle,
            onto: self.onto,
        })
    }

    fn again(saved: SavedWalk, _: &crate::doings::Moment) -> Option<Self> {
        Some(Self {
            target: saved.target,
            level: saved.level,
            speed: saved.speed,
            runs: saved.runs,
            flies: saved.flies,
            climbed: saved.climbed,
            moved: saved.moved,
            balked: saved.balked,
            within: saved.within,
            comes_on: saved.comes_on,
            idle: saved.idle,
            course: None,
            onto: saved.onto,
        })
    }
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
    /// Its course found again from where it stands at its next step, where it heads for a leg
    /// short of its target: a queue's walk sets its target again at each tick of the world while
    /// it moves, which its moving action takes for a new one and searches again (`0x59A885`,
    /// `0x4F5EFF`; docs/re/movement.md, "Searching again"), so that the leg moves on with it.
    pub fn steer(&mut self) {
        if self
            .course
            .as_ref()
            .is_some_and(|planned| planned.leg != self.target)
        {
            self.course = None;
        }
    }

    /// A walk to `target` from `level` at `speed`, in world units a second before the game's
    /// speed.
    pub fn to(target: Vec3, level: Level, speed: f32) -> Self {
        Self {
            target,
            level,
            speed,
            runs: false,
            flies: false,
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

    /// A flight to `target` from `level` at `speed`, a flyer's aloft (`minds::Gait::Flies`).
    pub fn fly(target: Vec3, level: Level, speed: f32) -> Self {
        Self {
            runs: true,
            flies: true,
            ..Self::to(target, level, speed)
        }
    }

    /// Whether it flies.
    pub fn flies(&self) -> bool {
        self.flies
    }

    /// The walk flown instead, at `speed`, the walker aloft (`flight::take_wing`).
    pub fn take_wing(&mut self, speed: f32) {
        self.runs = true;
        self.flies = true;
        self.speed = speed;
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
pub(crate) fn heading(degrees: f32) -> Vec3 {
    let (sin, cos) = degrees.to_radians().sin_cos();
    Vec3::new(cos + 2.0 * sin, 0.0, 2.0 * sin - cos).normalize()
}

/// The heading in degrees of a direction on the ground, [`heading`]'s inverse: its `x` and `z`
/// are `cos + 2 sin` and `2 sin − cos` of the heading at some scale (#3744).
pub(crate) fn degrees(direction: Vec3) -> f32 {
    let (sin, cos) = (
        (direction.x + direction.z) / 4.0,
        (direction.x - direction.z) / 2.0,
    );
    sin.atan2(cos).to_degrees()
}

/// The heading in degrees that a model turned by `rotation` faces ([`hero::facing`]'s model faces
/// +X in its bind pose).
pub(crate) fn heading_of(rotation: Quat) -> f32 {
    degrees(rotation * Vec3::X)
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

/// The cell of a `CreateObj` that gives its world position `0x20` and no cell `0x04`: the cell
/// under the point, on the layer of the position's third number. The client makes such an object
/// at the point itself (`0x485C10`: the tag `0x20` marks that no cell was given, and the object is
/// made by `sdk_OBJ_CREATE_NPOS` at `0x487695` in the sector it resolves the point to), as the
/// scripts make the big demon's summoning, the forges' elements and the glyphs (#2069).
fn world_cell(ops: &[opensacred_data::netscript::Operand]) -> Option<(i32, i32, i32)> {
    ops.iter().find_map(|o| match (o.tag, &o.value) {
        (WORLD_POSITION, Value::I32x3([x, y, layer])) => {
            let (cx, cy) = crate::paths::cell_of(*x as f32, *y as f32);
            Some((cx, cy, *layer))
        }
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

/// Whether an object that the start makes on `cell`, an effect of the type `effect` if it is one,
/// ends at once, the start's sector `start` (docs/re/effects.md, "The set-up: one system per
/// type", #3049): an effect of a type off the persistent list outside the 3 × 3 sectors about the
/// start, which the start makes at once in a sector that no `Enter` has marked (`+0x30` bit
/// `0x800`, `0x48907C`) and the simulation's tick ends at its first run (`0x5A6210`): the add-on's
/// temple door's spikes (1174), whose sector lies far from the add-on's start.
fn ends_at_start(start: (u32, u32), cell: (u32, u32), effect: Option<u32>) -> bool {
    let side = u32::try_from(SIDE).unwrap_or(64);
    let sector = (cell.0 / side, cell.1 / side);
    let outside = sector.0.abs_diff(start.0) > 1 || sector.1.abs_diff(start.1) > 1;
    outside && effect.is_some_and(|kind| !opensacred_rules::particles::persistent(kind))
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

/// What the sectors' `Enter` functions hold for their creatures ([`sectors()`]).
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
    /// Its `Weapon.pak` record's parent (`+0x24`), 0 for none: a hireling holds a weapon of a
    /// few parents in both hands whatever its kind (`opensacred_rules::equipment::grip`).
    parent: u32,
    model: Option<String>,
    texture: Option<String>,
    /// Whether its values count in its holder's: not where its `items.pak +0x03` has bit 1
    /// (`0x426550`, bit 25 of the record's first word), as the heroes' hair, the Gladiator's
    /// belt and the Mage's cowl have, which the stats' recomputation `0x5796A0` passes over (#490).
    counts: bool,
    /// The sound profile of its type (`items.pak +0x24`), its parent's where it has no slot of
    /// its own, as the heroes' starting weapons: the Seraphim's 7901 sounds as its parent 1724,
    /// the sword's profile 500 (`0x4257A0`, #2488).
    sound: u32,
    /// Its size (`items.pak +0x14`, 50 for 0, `arrows::size`): a held item's for the hurled
    /// blades' touch (`0x600340`, `0x428CE0`; `making::Blade`).
    size: f32,
    /// Its group of the armour switch (`items.pak +0x74`, `0x43CA20`), 0 for none.
    group: u16,
}

/// A `MakeGroup` rule (`0x4AB940`): the creature types it equips, its three items' pools and
/// chances in percent (`87`, `88`, `89`; a pool of −1 or a chance of 0 gives nothing), and its
/// spells (`67`, up to ten, each kept as a `CreateNPC`'s, [`script_spell`]), of which each
/// creature it equips takes one at random (#2949), and its name, the id of the text that its
/// last `1D` (a `res:` number), `1E` or `16` names (`+0x74`), which the creatures it names are
/// called by ([`called_of`], #3071).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Rule {
    types: Vec<u32>,
    items: [(i32, u32); 3],
    spells: Vec<u8>,
    name: Option<u32>,
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

/// The weapon pools as the original leaves them once the start script has run: the engine's
/// start (`0x46F9B0`, a new game's and a load's) then reads `bin\wea.bin` over the scripts' pools
/// (`0x470B20`), each of its lists the whole of its pool in order and an empty one emptying it
/// (#2651); `groups` its lists (`tables/weapon_groups.ron`), the scripts' pools alone without
/// them.
fn with_groups(
    mut pools: HashMap<i32, Vec<u32>>,
    groups: Option<&[Vec<u32>]>,
) -> HashMap<i32, Vec<u32>> {
    for (pool, items) in groups.unwrap_or_default().iter().enumerate() {
        let Ok(pool) = i32::try_from(pool) else {
            break;
        };
        if items.is_empty() {
            pools.remove(&pool);
        } else {
            pools.insert(pool, items.clone());
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
            let spells = ops
                .iter()
                .filter_map(|o| match (o.tag, &o.value) {
                    (ART, Value::Str(name)) => Some(script_spell(name)),
                    (ART, Value::I32(spell)) => u8::try_from(*spell).ok(),
                    _ => None,
                })
                .take(RULE_SPELLS)
                .collect();
            let name = ops
                .iter()
                .filter_map(|o| match (o.tag, &o.value) {
                    (0x1D, Value::I32(id)) => Some(id.to_string()),
                    (0x1E | 0x16, Value::Str(key)) => Some(key.clone()),
                    _ => None,
                })
                .next_back()
                .map(|key| opensacred_data::text::sacred_hash(key.as_bytes()));
            out.entry(sector).or_default().push(Rule {
                types,
                items,
                spells,
                name,
            });
        }
    }
    out
}

/// The most spells a `MakeGroup` rule carries (`0x4AB940`).
const RULE_SPELLS: usize = 10;

/// The text a creature of `creature` that a fill makes is called by: the name of the last of the
/// sector's rules that names its type and carries one, which `FillSector` puts in the creature's
/// key (`+0x3C`, `0x44A1B0` at `0x47F0C2`) as it walks every rule; none where none does, and the
/// creature is called by its type's name (`sdk_REFKEY`, `0x44A1C0`; [`Called`], #3071).
fn called_of(rules: &[Rule], creature: u32) -> Option<u32> {
    rules
        .iter()
        .filter(|rule| rule.types.contains(&creature))
        .filter_map(|rule| rule.name)
        .next_back()
}

/// The spell that a creature of `creature` takes by the first of the sector's rules that names
/// it, one of the rule's at random (`0x47F8E0`, `+0x2B6`; **hypothesis** for the roll, a draw of
/// the fill's dice after its items'); none where no rule names it or its rule has none (#2949).
fn spell_of(rules: &[Rule], creature: u32, rng: &mut spawning::Rng) -> Option<u8> {
    let rule = rules.iter().find(|r| r.types.contains(&creature))?;
    let count = u32::try_from(rule.spells.len()).ok().filter(|&n| n > 0)?;
    rule.spells
        .get(usize::try_from(rng.below(count)).unwrap_or(0))
        .copied()
}

/// The weapon kinds after which a creature takes no second item: the two-handed, the staves, the
/// bows and the crossbows (**hypothesis** for the names).
const ONE_ITEM: [u8; 5] = [2, 3, 4, 7, 12];

/// The items a creature of `creature` takes by the first of the sector's rules that names it
/// (`0x47F8E0`): each of the first two by its chance from its pool, at random among the pool's
/// items; a crossbow or a musket first that its model of the shots `shots` cannot shoot replaced
/// by `weapon` of the kind it asks for, none if it gives none (`spawning::filled_weapon`,
/// `0x480547`, #1217); then the two as its type's own list makes them
/// (`spawning::own_items`, #3070); the second only after a weapon of a kind that leaves a hand
/// free; the third, a worn piece, is [`worn_of`]'s (#2286).
fn gear_of<'a>(
    rules: &[Rule],
    creature: u32,
    (pools, gear): (&HashMap<i32, Vec<u32>>, &'a HashMap<u32, Gear>),
    (shots, weapon): (
        spawning::Shots,
        impl FnMut(u8, &mut spawning::Rng) -> Option<u32>,
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
    let first = roll(rule.items[0]);
    let second = roll(rule.items[1]);
    joined(creature, (first, second), (gear, shots), weapon, rng)
}

/// The first two items of a creature of `creature` as both the rules' path and the general
/// maker's join (`0x4805E0`): a crossbow or a musket first that its model of the shots `shots`
/// cannot shoot replaced by `weapon` of the kind it asks for (`spawning::filled_weapon`), the
/// two as its type's own list makes them (`spawning::own_items`), the second only after a weapon
/// of a kind that leaves a hand free.
fn joined<'a>(
    creature: u32,
    (first, second): (Option<u32>, Option<u32>),
    (gear, shots): (&'a HashMap<u32, Gear>, spawning::Shots),
    mut weapon: impl FnMut(u8, &mut spawning::Rng) -> Option<u32>,
    rng: &mut spawning::Rng,
) -> Vec<(u32, &'a Gear)> {
    let first = first.and_then(|item| {
        match spawning::filled_weapon(gear.get(&item).and_then(|g| g.kind), shots) {
            Some(kind) => weapon(kind, rng),
            None => Some(item),
        }
    });
    // The type's own list, which a first item already made enters (`0x4805E0`).
    let (first, second) = match first {
        Some(item) => {
            let (first, second) = spawning::own_items(creature, (item, second), rng);
            (Some(first), second)
        }
        None => (None, second),
    };
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

/// The third item of the first of the sector's rules that names a creature of `creature`, a worn
/// piece (`0x47F8E0`, #2286): by its chance from its pool, at random among the pool's items, as
/// the armour switch makes it for the creature's type by `switches` (`0x43CA20`): the item
/// itself, its variant, or nothing; the switch looks a derived item up by its root (up to six
/// parents within 2000 to 2999 and 6500 to 7999).
fn worn_of<'a>(
    rules: &[Rule],
    creature: u32,
    (pools, gear): (&HashMap<i32, Vec<u32>>, &'a HashMap<u32, Gear>),
    switches: &[Vec<[u32; 2]>],
    rng: &mut spawning::Rng,
) -> Option<(u32, &'a Gear)> {
    let rule = rules.iter().find(|r| r.types.contains(&creature))?;
    let (pool, chance) = rule.items[2];
    let made = rng.below(100) < chance;
    let list = pools.get(&pool).filter(|list| !list.is_empty())?;
    let index = usize::try_from(rng.below(u32::try_from(list.len()).ok()?)).ok()?;
    let item = made.then(|| list[index])?;
    switched(item, creature, gear, switches)
}

/// A worn piece `item` of a creature of `creature` as the armour switch makes it by `switches`
/// (`0x43CA20`): the item itself, its variant, or nothing; the switch looks a derived item up by
/// its root (up to six parents within 2000 to 2999 and 6500 to 7999).
fn switched<'a>(
    item: u32,
    creature: u32,
    gear: &'a HashMap<u32, Gear>,
    switches: &[Vec<[u32; 2]>],
) -> Option<(u32, &'a Gear)> {
    let derived = |item: u32| (2000..=2999).contains(&item) || (6500..=7999).contains(&item);
    let mut root = item;
    for _ in 0..6 {
        match gear.get(&root) {
            Some(g) if derived(root) && g.parent != 0 => root = g.parent,
            _ => break,
        }
    }
    let looked = gear.get(&root)?;
    let item = match opensacred_rules::equipment::armour_switch(
        creature,
        (looked.category, looked.group),
        switches,
    ) {
        Switched::Itself => item,
        Switched::Variant(variant) => variant,
        Switched::Nothing => return None,
    };
    Some((item, gear.get(&item)?))
}

/// The items that the general maker gives a creature of `creature` whose type no rule names, its
/// model of the clips' slots `slots` and the shots `shots` (`0x47F8E0` from `0x47FD20`;
/// docs/re/spawning.md, "The general maker", #3185): a weapon of the kind that its slots' draws
/// choose (`spawning::general_kind`) by `weapon`, the kind 1 with a shield by `shield` at its
/// draw (`spawning::general_shield`), two weapons as a weapon of the kind 1 in both hands; then
/// as the rules' path joins them ([`joined`]); and a worn piece of its type's own list
/// (`spawning::general_worn`) through the armour switch ([`switched`]).
#[expect(
    clippy::type_complexity,
    reason = "the makers' closures as the rules' path takes them"
)]
fn general_of<'a>(
    creature: u32,
    (slots, shots): (Option<&motions::Slots>, spawning::Shots),
    (gear, switches): (&'a HashMap<u32, Gear>, &[Vec<[u32; 2]>]),
    (mut weapon, mut shield): (
        impl FnMut(u8, &mut spawning::Rng) -> Option<u32>,
        impl FnMut(&mut spawning::Rng) -> Option<u32>,
    ),
    rng: &mut spawning::Rng,
) -> (Vec<(u32, &'a Gear)>, Option<(u32, &'a Gear)>) {
    let has = |slot| slots.is_some_and(|slots| slots.slot(slot).is_some());
    let (first, second) = match spawning::general_kind(creature, has, rng) {
        Some(spawning::KIND_TWO_WEAPONS) => {
            let first = weapon(spawning::BLADE, rng);
            (first, first)
        }
        Some(spawning::BLADE) => {
            let first = weapon(spawning::BLADE, rng);
            let second = first
                .filter(|_| spawning::general_shield(rng))
                .and_then(|_| shield(rng));
            (first, second)
        }
        Some(kind) => (weapon(kind, rng), None),
        None => (None, None),
    };
    let held = joined(creature, (first, second), (gear, shots), weapon, rng);
    let worn = spawning::general_worn(creature, rng)
        .and_then(|item| switched(item, creature, gear, switches));
    (held, worn)
}

/// What arms a creature by the `MakeGroup` rules ([`arm_by_rules`]): the weapon pools, the items'
/// gear, the armour switch's groups and the models' clips' slots.
type Armouries<'a> = (
    &'a HashMap<i32, Vec<u32>>,
    &'a HashMap<u32, Gear>,
    &'a [Vec<[u32; 2]>],
    &'a HashMap<String, motions::Slots>,
);

/// Arms `placement`, a creature of `creature`, by the rules `rules` as `0x47F8E0` does a fill's
/// and a `CreateNPC`'s of the operand `54` (#3072): its first two items ([`gear_of`]) in its hands
/// and its clips by what it holds, a crossbow its model cannot shoot another weapon by `weapon`
/// where `chooses` (#1217), and its third, a worn piece ([`worn_of`], #2286); a type that no rule
/// names by the general maker ([`general_of`], #3185), its weapons by `weapon` and its shield by
/// `shield`. It returns the items in its hands.
fn arm_by_rules(
    placement: &mut Placement,
    (rules, creature): (&[Rule], u32),
    (pools, gear, switches, slots): Armouries<'_>,
    (chooses, weapon, shield): (
        bool,
        impl FnMut(u8, &mut spawning::Rng) -> Option<u32>,
        impl FnMut(&mut spawning::Rng) -> Option<u32>,
    ),
    rng: &mut spawning::Rng,
) -> Vec<u32> {
    let model_slots = slots.get(&placement.model);
    let shots = match (chooses, model_slots) {
        (true, Some(slots)) => shots(slots),
        _ => spawning::Shots {
            crossbow: true,
            bow: true,
        },
    };
    let (held, worn) = if rules.iter().any(|rule| rule.types.contains(&creature)) {
        (
            gear_of(rules, creature, (pools, gear), (shots, weapon), rng),
            worn_of(rules, creature, (pools, gear), switches, rng),
        )
    } else {
        general_of(
            creature,
            (model_slots, shots),
            (gear, switches),
            (weapon, shield),
            rng,
        )
    };
    let worn_model = worn.and_then(|(item, worn)| {
        placement.worn = vec![(item, worn.category)];
        (!ATTACHED.contains(&worn.category)).then_some(())?;
        Some(Held {
            item,
            model: worn.model.clone()?,
            texture: worn.texture.clone(),
            hand: Hand::Right,
            mount: Mount::Worn,
            slot: opensacred_rules::equipment::slot_for(worn.category, |_| false),
        })
    });
    if !held.is_empty() {
        let categories: Vec<(u8, Option<u8>)> =
            held.iter().map(|(_, g)| (g.category, g.kind)).collect();
        placement.holding = InHand::of(held.iter().map(|&(item, g)| (item, g.category, g.kind)));
        placement.equipment = held
            .iter()
            .zip(hands(&categories))
            .filter_map(|(&(item, g), hand)| {
                Some(Held {
                    item,
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
        if let Some(slots) = slots.get(&placement.model) {
            placement.clips = ClipNames::of(slots, placement.held);
        }
    }
    placement.equipment.extend(worn_model);
    held.into_iter().map(|(item, _)| item).collect()
}

/// The group of the armour switch of an item of the record `record` (`items.pak +0x74`).
fn switch_group(record: &Item) -> u16 {
    u16::try_from(record.words_58[7] & 0xFFFF).unwrap_or(0)
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
    Some(spawning::Fill {
        per_area,
        entries,
        hostile: forced == Some(true),
        word: 0,
    })
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

/// Counts the respawn's gate's clock, on the world's time, which a pause stands (#3786).
fn tick(time: Res<Time>, npcs: Option<ResMut<Npcs>>) {
    if let Some(mut npcs) = npcs {
        npcs.kept.clock += time.delta_secs_f64();
    }
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

/// The trader of a stable's horse named by `name` (`0x63`, `SetOwner:`; `0x4624FC`): none for
/// an empty name, nor for `hero`, which gives the horse to the player (not followed).
fn trader_of(name: &str) -> Option<String> {
    (!name.is_empty() && !name.eq_ignore_ascii_case("hero")).then(|| name.to_owned())
}

/// The trader that a `CreateNPC`'s `0x63` makes its horse a stable's of ([`trader_of`],
/// `0x484E52`).
fn stock_of(ops: &[opensacred_data::netscript::Operand]) -> Option<String> {
    ops.iter().find_map(|o| match (o.tag, &o.value) {
        (STOCK, opensacred_data::netscript::Value::Str(name)) => trader_of(name),
        _ => None,
    })
}

/// A `CreateNPC`'s horse class, its tag `0x90` (`SetHorseClass:`), held to a byte; 0 for none.
fn horse_class(ops: &[opensacred_data::netscript::Operand]) -> u8 {
    ops.iter()
        .find(|o| o.tag == HORSE_CLASS)
        .and_then(|o| value_i32(&o.value))
        .and_then(|class| u8::try_from(class).ok())
        .unwrap_or(0)
}

/// The tag of a horse's class (`SetHorseClass:`, docs/re/horses.md).
const HORSE_CLASS: u8 = 0x90;

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
    /// The trader whose stock it joins, by its script name (#2300).
    stock: Option<String>,
    /// Whether it hires its creature as the hero's follower, or lets it go ([`hire_of`]; #657).
    hire: Option<crate::followers::Hire>,
    /// Whether its `0x6B` sets `+0x2B7` bit `0x08`, above 0, or clears it, for 0 ([`RISES`]), which
    /// keeps a dead follower of the hero to rise ([`RisesAgain`], #2459).
    rises: Option<bool>,
    /// Whether its `0xA1` marks the creature a vision ([`VISION`]; `0x461540` sets the bit and
    /// nothing of the record clears it).
    vision: bool,
    /// Whether its `0x80` lets the creature's body go ([`RELEASES`]): the flags word's `0x2000`
    /// cleared, its talk taken away, and a dead one's stamp put back so that its body vanishes a
    /// second later (`bodies::Released`).
    releases: bool,
    /// The seconds of the white butterflies that its `0x7F` shows over the creature's head
    /// ([`BUTTERFLIES`]), the last of them above 0; none for none.
    butterflies: Option<u32>,
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
            rises: if rises(ops) {
                Some(true)
            } else {
                listed(ops).then_some(false)
            },
            vision: ops.iter().any(|op| op.tag == VISION),
            releases: ops.iter().any(|op| op.tag == RELEASES),
            butterflies: butterflies(ops),
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
                (STOCK, Value::Str(name)) => {
                    state.stocked = true;
                    state.stock = trader_of(name);
                }
                _ => {}
            }
        }
        state
    }

    /// Whether it gives a creature made before what the game follows on one (#656, #658): its
    /// flags word (#2426), its hostility, its death's functions, its heading, its group, a
    /// trader's stock, a dialogue (#1028), a hire (#657), its rise (#2459) or a vision's mark
    /// (#3064).
    fn restates(&self) -> bool {
        self.word != 0
            || !self.deaths.is_empty()
            || self.hostile.is_some()
            || self.heading.is_some()
            || self.group.is_some()
            || self.stocked
            || self.talk.is_some()
            || self.hire.is_some()
            || self.rises.is_some()
            || self.vision
            || self.releases
            || self.butterflies.is_some()
    }

    /// The dialogue it leaves a creature that had `talk`: its own, or none for a horse it stocks.
    fn talk_of(&self, talk: Option<String>) -> Option<String> {
        let talk = self.talk.clone().unwrap_or(talk);
        talk.filter(|_| !self.stocked)
    }

    /// The flags word it gives a creature of the word `old` that trades by `trades`, the word
    /// its side is weighed by, and whether it takes the trades away: the mode `0x800` of a
    /// trader takes its trades rather than standing in its word (`0x461540`).
    fn flags_of(&self, old: u32, trades: u32) -> (u32, u32, bool) {
        let mut stated = self.word;
        let untrades = stated & 0x800 != 0 && trades != 0;
        if untrades {
            stated &= !0x800;
        }
        (ways::stated_word(old, stated), stated, untrades)
    }

    /// Gives the placement of a creature made before its flags word and its side by its
    /// hostility, as [`Self::apply`] gives them to one made with it (#2426).
    fn restate_flags(&self, placement: &mut Placement) {
        let (creature, _, word) = &mut placement.creature;
        let (flags, stated, untrades) = self.flags_of(*word, placement.trades);
        *word = flags;
        if untrades {
            placement.trades = 0;
        }
        if let Some(hostile) = self.hostile {
            placement.side = minds::stated_side(placement.side, *creature, stated, hostile);
        }
    }

    /// Gives `placement` its death's functions, its heading, its group, its stock's trader, its
    /// dialogue (none for one joining a stock), its hire, its sight function's bit, its rise and
    /// a vision's mark ([`Self::restates`]).
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
        if self.stock.is_some() {
            placement.stock.clone_from(&self.stock);
        }
        placement.talk = self.talk_of(placement.talk.take());
        if let Some(talk) = &self.talk {
            placement.dialogue = talk.is_some() && !self.stocked;
        }
        if self.hire.is_some() {
            placement.hire = self.hire;
        }
        // A sight function sets its bit with its entry (`0x4620EA`, #2475).
        if self.sights() {
            placement.creature.2 |= minds::SIGHT;
        }
        if let Some(rises) = self.rises {
            placement.rises = rises;
        }
        placement.vision |= self.vision;
    }

    /// Whether it gives its creature a sight function (`0x60`), which sets the flags word's
    /// [`minds::SIGHT`] (`0x4620EA`, #2475).
    fn sights(&self) -> bool {
        self.deaths.iter().any(|(replaces, _)| !replaces)
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
        let (flags, stated, untrades) = self.flags_of(*word, placement.trades);
        *word = flags;
        if untrades {
            placement.trades = 0;
        }
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

/// The seconds of the butterflies that the operands `ops` show ([`BUTTERFLIES`]): the last
/// `0x7F` of a number above 0.
fn butterflies(ops: &[opensacred_data::netscript::Operand]) -> Option<u32> {
    ops.iter()
        .rev()
        .find_map(|op| match (op.tag, &op.value) {
            (BUTTERFLIES, Value::I32(n)) => u32::try_from(*n).ok(),
            (BUTTERFLIES, Value::U16(n)) => Some(u32::from(*n)),
            (BUTTERFLIES, Value::U8(n)) => Some(u32::from(*n)),
            _ => None,
        })
        .filter(|&seconds| seconds > 0)
}

/// Whether the operands `ops` carry a [`BY_RULES`] of 0 or more, the last of them counting.
fn by_rules(ops: &[opensacred_data::netscript::Operand]) -> bool {
    ops.iter()
        .rfind(|op| op.tag == BY_RULES)
        .is_some_and(|op| matches!(op.value, Value::I32(v) if v >= 0))
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
/// `0x0A`, takes the members' away), its death's and sight functions, its level, the key of a
/// member of no name of its own and the rising again of its `0x6B` (#3148).
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
    /// Its `0x01`'s number above 0: the key (`+0x3C`) of a member that bears no name of its own,
    /// its `sdk_REFKEY` its type's (`0x49F0F7`–`0x49F10E`): the name its label and its talk's
    /// window show ([`Called`]).
    key: Option<u32>,
    /// Its `0x6B`: the `+0x2B7` bit 8 given ([`RisesAgain`]) by a number other than 0, taken by
    /// 0, left by none.
    rises: Option<bool>,
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
            key: ops
                .iter()
                .find_map(|o| match (o.tag, &o.value) {
                    (0x01, Value::I32(v)) => u32::try_from(*v).ok(),
                    (0x01, Value::U16(v)) => Some(u32::from(*v)),
                    (0x01, Value::U8(v)) => Some(u32::from(*v)),
                    _ => None,
                })
                .filter(|&key| key > 0),
            rises: if rises(ops) {
                Some(true)
            } else {
                listed(ops).then_some(false)
            },
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
        if let Some(key) = self.key
            && placement.name.is_none()
            && placement.called.is_none()
        {
            placement.called = Some(key);
        }
        if let Some(rises) = self.rises {
            placement.rises = rises;
        }
    }

    /// Gives a member in the world the change: its mind's flags word, side and dialogue, how it
    /// stands in the hero's way, and its functions; its level [`state_level`] gives it (#2894).
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
/// hero's for none, which `at` moves on the ground to a free point about the hero where another
/// creature stands in its cell (`0x45FF80`, `paths::about_hero`, #2858); one whose creature is
/// not found keeps its name, which places nothing.
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

/// A creature that a record of the play acts on, by its name, and the act ([`Act`]): an
/// `NPC_Goto`'s walk, a `Teleport`'s put or another's ([`Npcs::moves`]), at once or in its turn of
/// its queue under the anim mode (`queued`).
#[derive(Clone, Debug, PartialEq)]
struct Moved {
    name: String,
    act: Act,
    queued: bool,
}

/// What a record of the play does to a creature (#1220, #1277).
#[derive(Clone, Debug, PartialEq)]
enum Act {
    /// `NPC_Goto` (a walk) or `Teleport`, to `whither`; a walk of the tag `0x66` at a walk
    /// (`strolls`, [`Queued::Walk`]).
    Go {
        whither: Whither,
        walks: bool,
        strolls: bool,
    },
    /// `PlayAnim`: the slot of its clip, and how many times it plays outside the anim mode.
    Clip { slot: u16, loops: u32 },
    /// A `CallFunktion` naming the creature, which the scripts deferred under the anim mode: its
    /// function runs at its turn (#1278).
    Call(String),
    /// `Wait` under the anim mode: the other creature, `None` the hero, and its condition
    /// (#1279).
    Wait(Option<String>, Until),
    /// `PlaySound` under the anim mode, of the sound of the name (`0x4A9730`, the kind 14;
    /// #1826).
    Sound(String),
    /// `Partikel` under the anim mode whose second name is the creature's: the creature of its
    /// first name, at which its effect is made at the turn, and its numbers (`0x4A8BB0`, the kind
    /// 13; #1890).
    Partikel { at: String, numbers: Vec<i32> },
    /// `CreateObj` whose second `0x01` names the creature, under the anim mode: the record, made
    /// at the creature's turn (`0x486CB2`, the kind 11; #1826).
    Make(Box<Record>),
    /// `ActObj`: the creature opens or closes the door of the object of the name, at once or at
    /// its turn (`0x4A4310`, the kind 6; #1827).
    Obj { object: String, open: bool },
    /// `Morph` under the anim mode: the creature's new type, at its turn (`0x464170`, the kind 10;
    /// #1829).
    Morph(u32),
    /// `Attack` of `ECS_TELEPORT`: the creature's teleport to the place, at once or at its turn
    /// (`0x4A2B40`, the event `0x105`; #1831).
    Phase(Whither),
    /// `Attack` of a spell's art: the creature's cast of the spell at its target, at once or at its
    /// turn (the event `0x105`; #1871).
    Cast { spell: u16, at: CastAt },
    /// Another record that waits its turn under the anim mode, of the kind ([`QUEUED_OTHERS`]):
    /// its turn taken and no more, an `Attack` that neither teleports nor casts, which under the
    /// mode fights not (#1830).
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
/// or slot above 0, or for the name `schmiede`, which opens the smith's window instead. Of another
/// kind, what its record does to the creature it names, as told at each below: a deferred
/// `CallFunktion`, a `PlaySound`, a `Partikel`, a `Morph`, an `ActObj` or a `Wait`; else, under the
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
    // A `PlaySound` whose second name is the hero's or a creature's waits its turn under the
    // mode; outside it, or naming none, it plays at once as the line by name (`scripts`).
    if kind == PLAY_SOUND {
        let (sound, name) = (
            names.first()?,
            names.get(1).filter(|name| !name.is_empty())?,
        );
        return queued.then(|| Moved {
            name: name.clone(),
            act: Act::Sound(sound.clone()),
            queued,
        });
    }
    // A `Partikel` whose second name is not empty waits its turn under the mode in that one's
    // queue, its effect at its first name's creature (`0x4A8BB0`, #1890); at once it is the
    // scripts' (`scripts::Partikel`).
    if kind == PARTIKEL {
        let (first, second) = (
            names.first()?,
            names.get(1).filter(|name| !name.is_empty())?,
        );
        let numbers = ops
            .iter()
            .filter_map(|o| match (o.tag, &o.value) {
                (0x0B, Value::I32(n)) => Some(*n),
                _ => None,
            })
            .collect();
        return queued.then(|| Moved {
            name: second.clone(),
            act: Act::Partikel {
                at: first.clone(),
                numbers,
            },
            queued,
        });
    }
    // A `Morph` waits its turn under the mode alone; at once it is [`morphs`]'.
    if kind == MORPH {
        let name = names
            .first()
            .filter(|name| !name.is_empty() && *name != "hero")?;
        let creature = morphed_type(ops)?;
        if ops.iter().any(|o| o.tag == QUEUED_MORPH) {
            return None;
        }
        return queued.then(|| Moved {
            name: name.clone(),
            act: Act::Morph(creature),
            queued,
        });
    }
    // An `ActObj`: its creature first, the hero's too, its object second, both needed; at once
    // outside the mode, where its creature is in the world (`script_records`).
    if kind == ACT_OBJ {
        let (name, object) = (names.first()?, names.get(1)?);
        if name.is_empty() || object.is_empty() {
            return None;
        }
        let open = ops
            .iter()
            .rev()
            .find_map(|o| {
                if OPENS.contains(&o.tag) {
                    Some(true)
                } else if CLOSES.contains(&o.tag) {
                    Some(false)
                } else {
                    None
                }
            })
            .unwrap_or(true);
        return Some(Moved {
            name: name.clone(),
            act: Act::Obj {
                object: object.clone(),
                open,
            },
            queued,
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

/// The creature that a `CreateObj` names by its second `0x01`, lower case (`0x486CB2`): under
/// the anim mode its object is made at that creature's turn, where the creature is in the world
/// (#1826); none for another record or for the hero.
fn maker(record: &Record) -> Option<String> {
    let Body::Operands(ops) = &record.body else {
        return None;
    };
    if record.kind != CREATE_OBJ {
        return None;
    }
    ops.iter()
        .filter_map(|o| match (o.tag, &o.value) {
            (0x01, Value::Str(name)) => Some(name.to_ascii_lowercase()),
            _ => None,
        })
        .nth(1)
        .filter(|name| !name.is_empty() && name != HERO)
}

/// The indices of the `CreateObj`s of `records` that wait their turns in the queues of the
/// creatures they name ([`maker`]), those between a `SetAnimMode` or a `SetCinemaMode` and a
/// `SetPlayMode` that name a creature in the world (`present`), as [`Npcs::moves`] queues them:
/// the others are made at once.
fn deferred_objects(records: &[Record], present: impl Fn(&str) -> bool) -> HashSet<usize> {
    let mut queued = false;
    let mut deferred = HashSet::new();
    for (index, record) in records.iter().enumerate() {
        match record.kind {
            SET_ANIM_MODE | SET_CINEMA_MODE => queued = true,
            SET_PLAY_MODE => queued = false,
            _ => {}
        }
        if queued && maker(record).is_some_and(|name| present(&name)) {
            deferred.insert(index);
        }
    }
    deferred
}

/// Where a record's position of the tag `tag` sends a creature: a cell of its own, a `DefPos`'s
/// name, or a creature's place.
fn whither(
    ops: &[opensacred_data::netscript::Operand],
    tag: u8,
    named: &Places,
) -> Option<Whither> {
    ops.iter().find_map(|o| match (o.tag, &o.value) {
        (t, Value::I32x3([x, y, layer])) if t == tag => Some(Whither::Cell((*x, *y, *layer))),
        (t, Value::Name(name) | Value::Str(name)) if t == tag => {
            let name = name.to_ascii_lowercase();
            match name.strip_prefix(CREATURE_POSITION) {
                Some("hero") => Some(Whither::Creature(None)),
                Some(creature) => Some(Whither::Creature(Some(creature.to_owned()))),
                None => named
                    .get(&name)
                    .map(|place| Whither::Cell((place.cell.0, place.cell.1, place.level))),
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
/// dialogue's index is its entry's place from 1 and the index 0 gives none. It returns the items
/// that the records' `EquipNPC`s put on those placements, by their indices, in their order
/// (`0x481D80`, #2752), which [`Makers::equip`] puts on them.
fn set_states(
    records: &[&Record],
    made: &HashMap<*const Record, usize>,
    placements: &mut [Placement],
) -> Vec<(usize, Vec<u32>)> {
    let mut named: HashMap<String, Vec<usize>> = HashMap::new();
    let mut dialogues: Vec<String> = Vec::new();
    let mut equips = Vec::new();
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
            // Its items, each of its `0x02`s, onto the creature of its name.
            EQUIP_NPC => {
                let Some(indices) = name_of(ops).and_then(|name| named.get(&name)) else {
                    continue;
                };
                let items: Vec<u32> = ops
                    .iter()
                    .filter_map(|o| match (o.tag, &o.value) {
                        (0x02, Value::I32(v)) => u32::try_from(*v).ok(),
                        _ => None,
                    })
                    .collect();
                equips.extend(indices.iter().map(|&index| (index, items.clone())));
            }
            _ => {}
        }
    }
    equips
}

/// Where the records' `NPC_Goto`s send the NPCs they name, by name: the global cell and the
/// layer of the last that names each (`0x49E210`, which orders the creature to walk there), the
/// creature of a record's last name ([`mover`], #3240).
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
            Some((mover(NPC_GOTO, ops, true)?, target))
        })
        .collect()
}

/// The cells, global and their floors, that the records' `TriggerPatch`es (`0x496F20`) put under
/// a trigger that a `CreateTrigger` (`0x496520`) made closed: its tag [`CLOSED`] clears its
/// state's open bit, [`OPEN`] sets it, the last of them counting (a trigger of neither is open,
/// **hypothesis**, as the sector files' are). The client's cell test (`0x4EBCA0`) refuses such a
/// cell to every creature, the chests' and the barrels' among them (docs/re/movement.md, "The
/// objects in the way"); no script of the start opens them, but a door that names its trigger
/// does as it opens (#372). Each with its trigger's name, lower case.
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

/// The named positions of `records`: `DefPos` name and its place, of its first four numbers.
fn positions<'a>(records: impl IntoIterator<Item = &'a Record>) -> Places {
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
            let numbers: Vec<i32> = ops
                .iter()
                .skip(1)
                .map_while(|o| value_i32(&o.value))
                .take(4)
                .collect();
            Some((name.to_ascii_lowercase(), quests::Place::of(&numbers)?))
        })
        .collect()
}

/// A record's position (`0x04`): the global cell and the layer given as `(x, y, layer)`, or of a
/// `DefPos` name its place's cell and level, and the radius about it in which the record's
/// creature or object takes a cell of its own (`quests::Place::drawn`, #2442), 0 for a cell.
fn position(
    ops: &[opensacred_data::netscript::Operand],
    named: &Places,
) -> Option<((i32, i32, i32), i32)> {
    ops.iter().find_map(|o| match (o.tag, &o.value) {
        (0x04, Value::Name(name)) => named
            .get(&name.to_ascii_lowercase())
            .map(|place| ((place.cell.0, place.cell.1, place.level), place.radius)),
        (0x04, Value::I32x3([x, y, layer])) => Some(((*x, *y, *layer), 0)),
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
    /// The armour switch's groups (`bin/Rust.bin`, `armour_switches.ron`; `0x43CA20`), none
    /// from assets made before them.
    switches: Vec<Vec<[u32; 2]>>,
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

/// The places that the scripts' `DefPos`s name, by their names lower-cased: a global cell, a
/// radius and a level (`quests::Place`, #2442).
type Places = HashMap<String, quests::Place>;

/// What the play's records placed ([`Npcs::add`]): how many creatures and objects, and the
/// `RegionChange`s that the creatures in the world take ([`region_changes`]).
pub struct Added {
    pub placed: usize,
    pub changes: Vec<quests::AreaChange>,
    /// Each placement made, by the position of its record among the records and its index, by
    /// which a later `DelNPC` of the same run takes only those made before it ([`Npcs::regroup`]).
    pub born: Vec<(usize, usize)>,
    /// The items that the creatures of earlier records held before the records' `EquipNPC`s and
    /// after them, by their placements' indices ([`Npcs::equip_made`]), for those in the world
    /// (#2756).
    pub equipped: Vec<Equipped>,
    /// The placements of the items that go straight into the hero's backpack ([`TO_PACK`],
    /// #3543), which the play's run gives and a load's does not.
    pub into_pack: Vec<usize>,
}

/// A placement's index and the items its creature held before a run's `EquipNPC`s and after them
/// ([`Npcs::equip_made`], #2756).
pub type Equipped = (usize, Vec<u32>, Vec<u32>);

/// What the play changed of the scripts' placements and triggers, as a save keeps it (#1958):
/// the play's records of the world in the batches that [`script_records`] placed, in order, which
/// a load places again ([`Npcs::resume`]); the triggers open, those a hero turned and those
/// locked; the placements of the items taken and of the containers emptied, and the levels that
/// the lying items were made at (#1125).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
// Named apart from another saved type of its name, which the saves' layout test traces by
// name (#2306); postcard writes no names.
#[serde(rename = "SavedPlacements")]
pub struct Saved {
    pub records: Vec<Vec<Record>>,
    pub opened: HashSet<String>,
    pub marked: HashSet<String>,
    pub locked: HashSet<String>,
    pub taken: HashSet<usize>,
    pub emptied: HashSet<usize>,
    pub laid: HashMap<usize, u32>,
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
        let arming = self.arming(&items, creature.unwrap_or(0));
        // An NPC sent somewhere at the start walks there from where it was made.
        let target = name_of(ops).and_then(|name| targets.get(&name).copied());
        let (cell, spread) = match position(ops, named) {
            Some((cell, radius)) => (Some(cell), radius),
            None => (
                target.or_else(|| {
                    (record.kind == CREATE_OBJ)
                        .then(|| world_cell(ops))
                        .flatten()
                }),
                0,
            ),
        };
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
                (variant, height.unwrap_or(0.0), item.flags() & LIST_4 != 0)
            });
        // An element stone or a stargate of the FX objects, which loops its sound and has no
        // model of its item's.
        let fx_loop = (record.kind == CREATE_OBJ && category(item) == FX)
            .then(|| crate::fx_loops::sound_of(creature).map(|sound| (sound, creature)))
            .flatten();
        // A big demon's summoning of the FX objects, which has no model: its variant, the
        // record's `0x03` (`0x489FBA`), and its height.
        let summon = (record.kind == CREATE_OBJ
            && category(item) == FX
            && creature == DEMON_SUMMON)
            .then(|| {
                let variant = ops.iter().find_map(|o| match (o.tag, &o.value) {
                    (0x03, Value::U16(v)) => u8::try_from(*v).ok(),
                    _ => None,
                });
                let height = ops.iter().find_map(|o| match (o.tag, &o.value) {
                    (FX_HEIGHT, Value::I32(h)) => i16::try_from(*h).ok().map(f32::from),
                    _ => None,
                });
                (variant.unwrap_or(0), height.unwrap_or(0.0))
            });
        // A magic bar or barrier, or a bubble prison, of the FX objects, which has no model: its
        // type and its height.
        let prison = (record.kind == CREATE_OBJ
            && category(item) == FX
            && [MAGIC_PRISON, BUBBLE_PRISON].contains(&creature))
        .then(|| {
            let height = ops
                .iter()
                .find_map(|o| match (o.tag, &o.value) {
                    (FX_HEIGHT, Value::I32(h)) => i16::try_from(*h).ok().map(f32::from),
                    _ => None,
                })
                .unwrap_or(0.0);
            (creature, height)
        });
        // A dwarves' magic of the FX objects, which has no model: its variant and its height.
        let dwarf_magic = (record.kind == CREATE_OBJ && category(item) == FX)
            .then(|| DWARF_MAGICS.iter().position(|&kind| kind == creature))
            .flatten()
            .map(|k| {
                let height = ops.iter().find_map(|o| match (o.tag, &o.value) {
                    (FX_HEIGHT, Value::I32(h)) => i16::try_from(*h).ok().map(f32::from),
                    _ => None,
                });
                (u8::try_from(k + 1).unwrap_or(1), height.unwrap_or(0.0))
            });
        // An element of the forge's ritual of the FX objects: its variant and its height.
        let forge = (record.kind == CREATE_OBJ
            && category(item) == FX
            && opensacred_rules::particles::FORGE_ELEMENTS.contains(&creature))
        .then(|| {
            let height = ops.iter().find_map(|o| match (o.tag, &o.value) {
                (FX_HEIGHT, Value::I32(h)) => i16::try_from(*h).ok().map(f32::from),
                _ => None,
            });
            let first = *opensacred_rules::particles::FORGE_ELEMENTS.start();
            let variant = u8::try_from(creature - first).unwrap_or(0);
            (variant, height.unwrap_or(0.0))
        });
        // A trap or a hell gate of the scripts of the FX objects, which has no model: its type and
        // its height.
        let scripted = (record.kind == CREATE_OBJ
            && category(item) == FX
            && ([script_traps::FIELD, script_traps::FENCES].contains(&creature)
                || crate::hell_gates::Gate::is(creature)
                || crate::underworld_glyphs::Underworld::is(creature)
                || creature == opensacred_rules::shaddar::SLAVE
                || creature == opensacred_rules::zhadum::ZHADUM
                || creature == opensacred_rules::particles::BIG_DEMON_ILLUSION))
            .then(|| {
                let height = ops.iter().find_map(|o| match (o.tag, &o.value) {
                    (FX_HEIGHT, Value::I32(h)) => i16::try_from(*h).ok().map(f32::from),
                    _ => None,
                });
                (creature, height.unwrap_or(0.0))
            });
        // A piece of the scripts' Pong of the FX objects, which has no model: its height.
        let pong = (record.kind == CREATE_OBJ && category(item) == FX)
            .then(|| crate::pong::Piece::of(creature))
            .flatten()
            .map(|piece| {
                let height = ops.iter().find_map(|o| match (o.tag, &o.value) {
                    (FX_HEIGHT, Value::I32(h)) => i16::try_from(*h).ok().map(f32::from),
                    _ => None,
                });
                (piece, height.unwrap_or(0.0))
            });
        let empty = String::new();
        let Some(model) = self
            .models
            .get(&item.name.to_ascii_uppercase())
            .or((fx.is_some()
                || fx_loop.is_some()
                || summon.is_some()
                || prison.is_some()
                || dwarf_magic.is_some()
                || forge.is_some()
                || scripted.is_some()
                || pong.is_some())
            .then_some(&empty))
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
        let walk_floor = target.map_or(0, |(.., layer)| usize::try_from(layer).unwrap_or(0));
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
        // A sight function sets its bit with its entry (`0x4839E2`, #2475).
        let sights = record.kind == CREATE_NPC && ops.iter().any(|op| op.tag == ON_SIGHT);
        let word = crowd::flags_word(markers()) | if sights { minds::SIGHT } else { 0 };
        let class = self.classes.get(&creature).copied().unwrap_or_default();
        let hostility = minds::created_hostility(markers());
        let side = minds::created_side(class, creature, word, hostility);
        let trades = if record.kind == CREATE_NPC {
            markers().fold(0, |trades, tag| trades | trade(tag))
        } else {
            0
        };
        Ok(Some(Placement {
            born: None,
            model: model.clone(),
            woman: item.name.contains(WOMAN),
            horse_class: if record.kind == CREATE_NPC {
                horse_class(ops)
            } else {
                0
            },
            stock: (record.kind == CREATE_NPC).then(|| stock_of(ops)).flatten(),
            rides: false,
            cell: (x, y),
            drawn: if record.kind == CREATE_OBJ {
                drawn_at(ops)
            } else {
                None
            },
            home: None,
            walk,
            walk_floor,
            speed: [walking, running].map(f32::from),
            class,
            base: self.bases.get(&creature).copied().unwrap_or_default(),
            held: arming.class,
            ranged: arming.ranged,
            size: arrows::size(item.words_14[0]),
            made: (origin.enter, true),
            started: origin.started,
            ended: false,
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
            special: record.kind == CREATE_OBJ && category(item) == SPECIAL,
            fx,
            fx_loop,
            summon,
            prison,
            dwarf_magic,
            forge,
            scripted,
            pong,
            trigger: (record.kind == CREATE_OBJ && [DOOR, CHEST].contains(&category(item)))
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
            emerges: record.kind == CREATE_NPC && markers().any(|tag| tag == EMERGES),
            called: None,
            sinks: record.kind == CREATE_OBJ
                && category(item) != CHEST
                && (category(item) == SINKING || self.kinds.contains_key(&creature)),
            vision: record.kind == CREATE_NPC && ops.iter().any(|op| op.tag == VISION),
            texture,
            layer,
            spread,
            by_rules: None,
            items,
            equipment: arming.equipment,
            holding: arming.holding,
            worn: arming.worn,
            rank: fight::Rank::of_tag(tag),
            tag,
            clips,
            plain: false,
            name: name_of(ops),
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
                && takeable(item)
                && !ops.iter().any(|o| o.tag == QUEST_ITEM),
            to_pack: record.kind == CREATE_OBJ && ops.iter().any(|o| o.tag == TO_PACK),
            taking: (record.kind == CREATE_OBJ)
                .then(|| {
                    ops.iter().find_map(|o| match (o.tag, &o.value) {
                        (ON_TAKE, Value::Str(name)) => Some(name.clone()),
                        _ => None,
                    })
                })
                .flatten(),
            raised: record.kind == CREATE_OBJ && ops.iter().any(|o| o.tag == RAISED),
            quest: (record.kind == CREATE_OBJ
                && takeable(item)
                && ops
                    .iter()
                    .any(|o| o.tag == QUEST_ITEM && matches!(o.value, Value::Marker)))
            .then(|| {
                ops.iter().find_map(|o| match (o.tag, &o.value) {
                    (ON_TAKE, Value::Str(name)) => Some(name.clone()),
                    _ => None,
                })
            }),
            container: (record.kind == CREATE_OBJ && category(item) == CHEST).then(|| {
                ops.iter().find_map(|o| match (o.tag, &o.value) {
                    (ON_TAKE, Value::Str(name)) => Some(name.clone()),
                    _ => None,
                })
            }),
            uses: (record.kind == CREATE_OBJ)
                .then(|| {
                    ops.iter().find_map(|o| match (o.tag, &o.value) {
                        (ON_USE, Value::Str(name)) => Some(name.clone()),
                        _ => None,
                    })
                })
                .flatten(),
            spell: if record.kind == CREATE_NPC {
                ops.iter()
                    .rev()
                    .find_map(|o| match (o.tag, &o.value) {
                        (ART, Value::Str(name)) => Some(script_spell(name)),
                        _ => None,
                    })
                    .unwrap_or(0)
            } else {
                0
            },
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

    /// Puts the items that `EquipNPC`s put on placements (`equips`, of [`set_states`]) on them, after
    /// those they hold as [`Self::equipped`] leaves them (`0x481D80`): each of them equipped as a
    /// `CreateNPC`'s items are (`0x54B130`), what it holds and wears and its clips made anew by
    /// them (#2752).
    fn equip(
        &self,
        equips: Vec<(usize, Vec<u32>)>,
        placements: &mut [Placement],
        model_slots: &mut HashMap<String, motions::Slots>,
    ) -> Result<()> {
        for (index, items) in equips {
            let Some(placement) = placements.get_mut(index) else {
                continue;
            };
            let held = self.equipped(&placement.items, &items);
            if let Some(equipped) =
                self.remade(placement, placement.creature.0, &held, model_slots)?
            {
                *placement = equipped;
            }
        }
        Ok(())
    }

    /// The items that a creature holding `held` holds after an `EquipNPC` of `items` (`0x481D80`,
    /// #2756): before each item, a weapon in a hand ([`hands`]) of the kinds 2, 3, 4, 7 or 12, or
    /// of the types 4067 to 4072, is taken out of it (`0x5CECD0`, `0x556F10`), and the item goes
    /// after the others (`0x54B130`, the hand by [`hands`]).
    fn equipped(&self, held: &[u32], items: &[u32]) -> Vec<u32> {
        let mut equipped = held.to_vec();
        for &item in items {
            let categories: Vec<(u8, Option<u8>)> = equipped
                .iter()
                .map(|held| {
                    (
                        self.record(*held).map_or(0, category),
                        self.kinds.get(held).copied(),
                    )
                })
                .collect();
            let in_hands = hands(&categories);
            let mut kept = Vec::with_capacity(equipped.len());
            for ((held, (category, kind)), hand) in equipped.iter().zip(&categories).zip(in_hands) {
                if hand.is_none() || !taken_out(*held, *category, *kind) {
                    kept.push(*held);
                }
            }
            kept.push(item);
            equipped = kept;
        }
        equipped
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
        let arming = self.arming(items, creature);
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
        remade.worn = arming.worn;
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
    fn arming(&self, items: &[u32], creature: u32) -> Arming {
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
            .filter_map(|(&(id, item), hand)| {
                let model = self.models.get(&item.name.to_ascii_uppercase())?;
                let texture = match item.words_08[0] {
                    0 => None,
                    number => self.textures.get(&number).cloned(),
                };
                Some(Held {
                    item: id,
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
        // The items that go into no hand it wears, as the armour switch makes them for its type
        // (`0x54B130`, `0x555300` asking `0x43CA20`, #2286); an item switched to nothing is gone.
        let mut equipment: Vec<Held> = equipment;
        let mut worn = Vec::new();
        for ((&(item, record), hand), &(category, _)) in
            equipped.iter().zip(hands(&categories)).zip(&categories)
        {
            if hand.is_some() || matches!(category, WEAPON | SHIELD) {
                continue;
            }
            let item = match opensacred_rules::equipment::armour_switch(
                creature,
                (category, switch_group(record)),
                &self.switches,
            ) {
                Switched::Itself => item,
                Switched::Variant(variant) => variant,
                Switched::Nothing => continue,
            };
            let Some(record) = self.record(item) else {
                continue;
            };
            worn.push((item, category));
            // A horse's bridle goes into its slot 1, which no draw reads (`0x54ACC0`; a stable's
            // horses stand bare, docs/re/horses.md, "The horse's button, the call and the horse
            // left alone"); its values count.
            if category == crate::riding::BRIDLES {
                continue;
            }
            if let Some(held) = self.worn_model(item, record) {
                equipment.push(held);
            }
        }
        Arming {
            equipment,
            holding,
            worn,
            class: held_class(&categories),
            ranged: ranged(&categories),
        }
    }

    /// The model that a creature wears of the item `item` of the record `record` on its
    /// skeleton, in the first slot of its category; none for a ring, an amulet or the back's,
    /// which a creature carries on a bone of its own, not on its skeleton ([`ATTACHED`]), or
    /// without a model.
    fn worn_model(&self, item: u32, record: &Item) -> Option<Held> {
        let category = category(record);
        if ATTACHED.contains(&category) {
            return None;
        }
        Some(Held {
            item,
            model: self.models.get(&record.name.to_ascii_uppercase())?.clone(),
            texture: match record.words_08[0] {
                0 => None,
                number => self.textures.get(&number).cloned(),
            },
            hand: Hand::Right,
            mount: Mount::Worn,
            slot: opensacred_rules::equipment::slot_for(category, |_| false),
        })
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

    /// A shield of any kind at the level `level` as `0x5C7FA0` chooses the general maker's
    /// (#3185): of the category 13, of the quality 14, of any class and no set piece.
    fn shield(&self, level: u32, dice: &mut impl spawning::Dice) -> Option<u32> {
        let wanted = opensacred_rules::drops::Wanted {
            category: SHIELD,
            level,
            class: 0,
            loose: false,
            quality: spawning::REARMED_QUALITY,
            kind: None,
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
    worn: Vec<(u32, u8)>,
    class: u8,
    ranged: bool,
}

impl Npcs {
    /// The `Creature.pak` flags of the type `creature` (`+6`, `0x43AD80`), 0 for a type without
    /// a model: the ghosts' `0x20` (`opensacred_rules::undead::GHOST_FLAG`).
    pub(crate) fn table_flags(&self, creature: u32) -> u8 {
        self.types.get(&creature).map_or(0, |kind| kind.flags)
    }

    /// The `Creature.pak` walk and run speeds of the type `creature` (`+0x26`, `+0x28`).
    pub(crate) fn speeds_of(&self, creature: u32) -> [u16; 2] {
        self.makers
            .speeds
            .get(&creature)
            .copied()
            .unwrap_or_default()
    }

    /// The horse data's row of the class `class` (#2299).
    pub(crate) fn horse_row(&self, class: u8) -> opensacred_rules::horses::Row {
        opensacred_rules::horses::Row::of(&self.horses, class)
    }

    /// The level that a script's `CreateObj` makes an item at the place `at` at for the hero of
    /// level `hero`, by the levels of the sector there (`opensacred_rules::fight::Rules::
    /// script_level`, #1125).
    pub(crate) fn script_level(&self, hero: u32, at: Vec3) -> u32 {
        self.fight
            .script_level(hero, self.values.get(&sector_of(at)))
    }

    /// The level that a quest's reward makes its item at for the hero of level `hero` standing at
    /// `at`, by the levels of the sector there (`opensacred_rules::fight::Rules::reward_level`,
    /// #2872).
    pub(crate) fn reward_level(
        &self,
        hero: u32,
        at: Vec3,
        dice: &mut impl opensacred_rules::spawning::Dice,
    ) -> u32 {
        self.fight
            .reward_level(hero, self.values.get(&sector_of(at)), dice)
    }

    /// The levels of the sector at the place `at`, none for a sector without them, and the fight's
    /// rules that raise them by the difficulty, by which a container there holds its items' levels
    /// (`opensacred_rules::fight::Rules::contained_level`, #2858).
    pub(crate) fn levels_at(&self, at: Vec3) -> (Option<&spawning::SpawnValues>, &fight::Rules) {
        (self.values.get(&sector_of(at)), &self.fight)
    }

    /// The level that a container's gold is made for, for the hero of level `hero` standing at
    /// `at` (`opensacred_rules::fight::Rules::gold_level`, #2858).
    pub(crate) fn gold_level(&self, hero: u32, at: Vec3) -> u32 {
        self.fight.gold_level(hero, self.values.get(&sector_of(at)))
    }

    /// The level of a horse that a trader's stock makes again at the place `at` for the hero of
    /// level `hero`, of the roll `roll` (`opensacred_rules::fight::Rules::restock_level`,
    /// #2320).
    pub(crate) fn restock_level(&self, hero: u32, at: Vec3, roll: u32) -> u32 {
        self.fight
            .restock_level(hero, self.values.get(&sector_of(at)), roll)
    }

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
        let switches: Vec<Vec<[u32; 2]>> = files::read_to_string(
            assets
                .join("tables")
                .join(opensacred_data::tables::ARMOUR_SWITCHES),
        )
        .ok()
        .and_then(|text| ron::from_str(&text).ok())
        .unwrap_or_default();
        let makers = Makers {
            assets: assets.clone(),
            switches,
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
        let equips = set_states(&records, &made, &mut placements);
        makers.equip(equips, &mut placements, &mut model_slots)?;
        // The start's objects outside the 3 × 3 about its `StartPosition`, the sectors that no
        // `Enter` has marked yet (`+0x30` bit `0x800`): its effects of the types off the
        // persistent list there end at once (#3049). Its chests, barrels and the like there, which
        // its `CreateObj`s keep in the interpreter's list `+0x364` for their sectors' `Enter` to
        // make each time (`0x486AA7`, #2292), are placed as any other, the emptied ones by their
        // placements (#1380; docs/re/fight.md, "The containers that an `Enter` makes again").
        if let Some(((x, y), _)) = start_position(tree) {
            let side = u32::try_from(SIDE).unwrap_or(64);
            let start = (x / side, y / side);
            for placement in placements.iter_mut().filter(|p| p.object && p.started) {
                let kind = placement.creature.0;
                let effect = items.get(&kind).is_some_and(|item| category(item) == FX);
                placement.ended = ends_at_start(start, placement.cell, effect.then_some(kind));
                if placement.ended {
                    bevy::log::debug!(
                        "npcs: the start's effect {kind} at {:?} ends at once, its sector not \
                         marked",
                        placement.cell
                    );
                }
            }
        }
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
                        woman: item.name.contains(WOMAN),
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
        // The scripts' pools, which `wea.bin`'s lists replace (#2651).
        let groups: Option<Vec<Vec<u32>>> = files::read_to_string(
            assets
                .join("tables")
                .join(opensacred_data::tables::WEAPON_GROUPS),
        )
        .ok()
        .and_then(|text| ron::from_str(&text).ok());
        let pools = with_groups(pools(&records), groups.as_deref());
        // The pools' items, the weapons that an archer takes to melee and back (#396), the
        // hero's (#486), and every type of a `Weapon.pak` record: the bows and the weapons of
        // kind 1 that a crossbow its holder cannot shoot becomes (#1217), and whatever weapon,
        // shield or piece the hero picks up and puts into its hands or on (#1416).
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
                .chain(weapons.iter().map(|w| &w.id)),
        );
        let side = u32::try_from(SIDE)?;
        let mut by_sector: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (index, placement) in placements.iter().enumerate().filter(|(_, p)| !p.ended) {
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
            horses: balance
                .iter()
                .flat_map(|balance| balance.tail_4.iter().map(|row| (row.key, row.words)))
                .collect(),
            places: positions,
            pending: Vec::new(),
            played: HashSet::new(),
            records: Vec::new(),
            taken: HashSet::new(),
            removed: HashSet::new(),
            emptied: HashSet::new(),
            laid: HashMap::new(),
            dialogues,
            icons,
            region_talks: region_talks(&records),
            values,
            fills,
            templates,
            types,
            classes,
            factors: factors(balance),
            respawns: respawns(balance),
            fight: fight::Rules::at(balance, difficulty),
            items: fight::Templates::new(&modifiers, weapons).with_balance(balance),
            rules: rules(tree),
            pools,
            gear,
            slots: model_slots,
            hits: hit_fractions(tables),
            points: hit_points(tables),
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
            last_enter: None,
            kept: Kept::default(),
            queues: HashMap::new(),
            entered: HashMap::new(),
            waters: Vec::new(),
            drawn: HashSet::new(),
            riding: 0,
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

    /// Whether an item of the type `item` in a hand lays an art's streak (`weapon_streaks`): a
    /// weapon or a cannon whose values count, not a cosmetic item (`0x426550`,
    /// `opensacred_rules::particles::streaks_on`).
    pub(crate) fn streaked(&self, item: u32) -> bool {
        self.gear.get(&item).is_some_and(|gear| {
            gear.counts && opensacred_rules::particles::streaks_on(gear.category)
        })
    }

    /// How a creature holds an item of the type `item` (`opensacred_rules::equipment::grip`): a
    /// weapon by its kind and its record's parent, else its own type; a shield in the left hand
    /// alone; none for another item.
    pub(crate) fn grip(&self, item: u32) -> Option<equipment::Grip> {
        let gear = self.gear.get(&item)?;
        let root = if gear.parent == 0 { item } else { gear.parent };
        match gear.category {
            WEAPON => Some(equipment::grip(gear.kind.unwrap_or(0), root)),
            SHIELD => Some(equipment::Grip::default()),
            _ => None,
        }
    }

    /// Whether a hireling of `arms` takes an item of the type `item` from the cursor through its
    /// portrait (`0x6D6B90`): a shield, or a weapon whose kind's first attack's slot its model
    /// has (`0x956F80`: 79 + 5 · kind for the kinds 0 to 12, 241 for 13, 139 for 14;
    /// `0x4117B0`).
    pub(crate) fn takes_through_portrait(&self, arms: &Arms, item: u32) -> bool {
        let Some(gear) = self.gear.get(&item) else {
            return false;
        };
        match gear.category {
            SHIELD => true,
            WEAPON => {
                let slot = match gear.kind.unwrap_or(0) {
                    kind @ 0..=12 => 79 + 5 * u16::from(kind),
                    13 => 241,
                    14 => 139,
                    _ => return false,
                };
                self.slots
                    .get(&arms.model)
                    .is_some_and(|slots| slots.slot(slot).is_some())
            }
            _ => false,
        }
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
            sets: Default::default(),
            // On foot as it starts; `equipment::refit` seats it.
            mounted: false,
            class,
            two: two_weapons_of(&holding, class),
            // Worked out of their values; `equipment::refit` gives the worn items' (#3080).
            levels: Vec::new(),
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
            item: held.item,
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
            // A gun in the right hand, which the shot's maker asks (`0x4DCB80`, #1963).
            gun: holding.iter().any(|held| {
                held.hand == Hand::Right
                    && held.category == WEAPON
                    && held.kind.is_some_and(|kind| GUNS.contains(&kind))
            }),
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
                    item: held.item,
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

    /// The model, the texture and the size of the item of type `item` in a hand, if it has a
    /// model (a thrown dagger's, #2575).
    pub fn held_model(&self, item: u32) -> Option<(&str, Option<&str>, f32)> {
        let gear = self.gear.get(&item)?;
        Some((gear.model.as_deref()?, gear.texture.as_deref(), gear.size))
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

/// The weapons whose sounds a type of no sound profile takes by its family (`Weapon.pak +0x85`,
/// `0x425EC0`'s switch, #2496): five each, one of which the type's own seed picks ([`by_family`]);
/// none for the other families.
fn family_weapons(family: u8) -> Option<[u32; 5]> {
    Some(match family {
        0 => [1724, 1725, 1726, 1728, 1729],
        1 => [1727, 1712, 1713, 1714, 1712],
        5 | 6 => [1722, 1723, 1737, 1738, 1754],
        8 | 9 => [1743, 1731, 1730, 1732, 1733],
        10..=12 => [1862, 1863, 1864, 1865, 1866],
        21..=24 => [1875, 1852, 1717, 1716, 1850],
        _ => return None,
    })
}

/// The first `rand()` of the C library after `srand(seed)` (`0x849B33`, `0x849B40`): the state
/// times 214,013 plus 2,531,011, its bits 16 to 30.
fn first_rand(seed: u32) -> u32 {
    (seed.wrapping_mul(214_013).wrapping_add(2_531_011) >> 16) & 0x7FFF
}

/// The sound profile that a type `slot` of no profile of its own takes (`0x425EC0`, #2496): below
/// 8,191 and of a `Weapon.pak` record, a weapon of its record's family ([`family_weapons`]),
/// picked by `rand() % 5` after `srand(slot)`, and that weapon's profile; 0 for any other.
fn by_family(
    slot: u32,
    weapons: &HashMap<u32, &opensacred_data::tables::Weapon>,
    items: &HashMap<u32, &Item>,
) -> u32 {
    if slot >= 0x1FFF {
        return 0;
    }
    let Some(record) = weapons.get(&slot) else {
        return 0;
    };
    let family = u8::try_from(record.words_84.first().copied().unwrap_or(0) >> 8).unwrap_or(0);
    let Some(choice) = family_weapons(family) else {
        return 0;
    };
    let pick = usize::try_from(first_rand(slot) % 5).unwrap_or(0);
    items.get(&choice[pick]).map_or(0, |item| item.sound())
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
    let records: HashMap<u32, &opensacred_data::tables::Weapon> =
        weapons.iter().map(|w| (w.id, w)).collect();
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
                parent: parents.get(&slot).copied().unwrap_or(0),
                model: models.get(&item.name.to_ascii_uppercase()).cloned(),
                // `FX_OPAQUE` the texture of a type drawn by its effect (#1834), else left out.
                texture: match item.words_08[0] {
                    0 => None,
                    FX_OPAQUE if item.model_effect().is_none() => None,
                    number => textures.get(&number).cloned(),
                },
                counts: item.byte_03 & 2 == 0,
                // Its parent's where it has no slot of its own, as the items' table's loader
                // copies a parent's entry onto its children's (`0x4257A0` at `0x425A1A`, #2488);
                // where that is none, a weapon's of its family (#2496).
                sound: match item.sound() {
                    0 => by_family(slot, &records, items),
                    sound => sound,
                },
                size: arrows::size(item.words_14[0]),
                group: switch_group(item),
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
                item,
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
/// and texture (#1131), in the first slot that takes its category (#1307). A template's are the
/// class's own cosmetic items, which its default
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
                item: worn.item,
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

    /// What a creature of `request` is placed as ([`spawn_creatures`]): its type's template
    /// ([`Self::template`]) as `CreateNPC` makes it, on the request's floor, of its rank.
    fn requested(&mut self, request: &Spawn, assets: &Path) -> Result<Placement, String> {
        let creature = request.creature;
        let mut placement = self.template(creature, assets)?;
        let critter = crowd::constructed(creature, placement.creature.1);
        // Hostile or peaceful by its class's side, as the creature maker makes it for neither
        // (`0x480D40`'s −1 asks `0x549000` whether it is hostile to the local hero, at
        // `0x480FD5`).
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
        Ok(placement)
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
    /// horse's by the hero's riding and any other's by the survival counter, `played`, held at the
    /// sector's highest (#562); its roll the same each time it is made on its cell.
    fn scripted_level(&self, placement: &Placement, hero: u32, played: fight::Played) -> u32 {
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
            // A start's horse by a first-level hero's riding, which no class's template holds.
            let riding = if placement.started { 0 } else { self.riding };
            fight::Raise::Horse { riding }
        } else {
            fight::Raise::Played(played)
        };
        let mut dice = spawning::Rng::of(x, y, LEVEL_ROLL);
        fight::scripted_level(values, hero, raise, creature, &self.fight, &mut dice)
    }
}

/// Keeps [`Npcs`]' level of the hero's riding, the skill 22 of its arts' skills, by which a
/// script's horse is made ([`Npcs::scripted_level`], #2814).
fn hero_riding(arts: Option<Res<crate::arts::HeroArts>>, mut npcs: ResMut<Npcs>) {
    let riding = arts.as_deref().map_or(0, |arts| {
        arts.skills
            .iter()
            .find(|&&(skill, _)| skill == RIDING)
            .map_or(0, |&(_, level)| level)
    });
    if npcs.riding != riding {
        npcs.riding = riding;
    }
}

/// The riding's skill, whose level raises a script's horse (`0x4266F0`).
const RIDING: u8 = 22;

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
    (played, holding): (
        Res<drops::Survival>,
        Query<&fight::Holding, With<hero::Body>>,
    ),
) -> Result<Vec<Entity>, String> {
    let npcs = &mut *npcs;
    let mut placement = npcs.requested(&request, &data.0.assets)?;
    let hero = experience.level;
    let side = u32::try_from(SIDE).unwrap_or(64);
    let mut made = Vec::new();
    for &cell in &request.cells {
        placement.cell = cell;
        let at = (cell.0 / side, cell.1 / side);
        let values = npcs.values.get(&at).copied().unwrap_or_default();
        let level = request.level.unwrap_or_else(|| {
            let time = played.time(holding.iter().next());
            let level = npcs.scripted_level(&placement, hero, time);
            let big = placement.creature.1 & crowd::BIG != 0;
            placement
                .rank
                .level(level, (hero, time), &values, big, &npcs.fight)
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
            commands.entity(entity).insert((
                bevy::prelude::DespawnOnExit(crate::game::AppState::InGame),
                Named(Id::Born(npcs.kept.mint())),
            ));
            made.push(entity);
            continue;
        }
        let placed = Placed {
            at,
            source: Source::Filled(Box::new(placement.clone())),
        };
        npcs.kept.spawned(&placed);
        commands.entity(entity).insert(placed);
        streamer.adopt(&mut commands, at, entity);
        made.push(entity);
    }
    Ok(made)
}

/// The drops' tables and the difficulty by which a script's item lying is made (#1125).
type Laying<'a> = (&'a drops::Tables, opensacred_rules::drops::Difficulty);

/// A script's item of type `item` as its type's record holds it, made by no item maker.
fn plain_item(item: u32) -> opensacred_rules::drops::Dropped {
    opensacred_rules::drops::Dropped {
        kind: opensacred_rules::drops::Kind::Other,
        item,
        level: 0,
        key: QUEST_KEY,
        quality: 0,
        amount: 1,
        values: None,
        named: None,
        socketed: Vec::new(),
        name_key: 0,
        script: None,
    }
}

/// The item that the `CreateObj` `placement` of the index `index` lays, as the rules make it
/// (`drops::scripted`, `0x487AB0`; #1125) by `made`: at the level of the hero then, `hero`,
/// held within the levels `values` of its sector by `rules`, a start's at the hero's first
/// level, as its records ran then. That level is kept in `laid` the first time, but for an
/// `Enter`'s item, which its sector makes again at each coming, and the rolls are the
/// placement's own, so that each coming lays the same item, as the original keeps its object.
/// The item as its type's record holds it where the rules leave it so, or without the tables.
fn laid_item(
    (laid, values, rules): (
        &mut HashMap<usize, u32>,
        Option<&spawning::SpawnValues>,
        &fight::Rules,
    ),
    (index, placement): (usize, &Placement),
    (hero, made): (u32, Option<Laying>),
) -> opensacred_rules::drops::Dropped {
    let item = placement.creature.0;
    let Some((tables, difficulty)) = made else {
        return plain_item(item);
    };
    // A pile of gold, its record's price its amount, which the pick-up adds to the purse
    // (`0x5D3AB0`'s first step, the files' 600; seen: quest 1301's 5135 gave 600; #3090).
    if opensacred_rules::drops::GOLD.contains(&item) {
        return opensacred_rules::drops::Dropped {
            kind: opensacred_rules::drops::Kind::Gold,
            amount: u32::try_from(tables.price(item)).unwrap_or(0),
            ..plain_item(item)
        };
    }
    let hero = if placement.started {
        fight::HERO_LEVEL
    } else {
        hero
    };
    let level = if placement.made.0 {
        rules.script_level(hero, values)
    } else {
        *laid
            .entry(index)
            .or_insert_with(|| rules.script_level(hero, values))
    };
    let mut dice = spawning::Rng::of(placement.cell.0, placement.cell.1, index);
    drops::scripted(
        tables,
        difficulty,
        (item, level, placement.raised),
        &mut dice,
    )
    .unwrap_or_else(|| plain_item(item))
}

/// Places the placement `index` of the sector `at` as it comes in: a kept one anew the first time,
/// where it stood after and never once killed, the others anew each time. Made anew, a weapon
/// that its model cannot shoot is chosen anew by `armoury` where it is read (#1217), which a kept
/// one keeps.
fn place_index(
    commands: &mut Commands,
    npcs: &mut Npcs,
    (index, at): (usize, (u32, u32)),
    (heights, states, assets, crowd): (
        &Heights,
        &buildings::Buildings,
        &AssetServer,
        Option<&crowd::Crowd>,
    ),
    (hero, time, area_types): (u32, fight::Played, Option<&quests::AreaTypes>),
    (armoury, made): (Option<Armoury>, Option<Laying>),
    streamer: &mut Streamer,
) {
    // An item that the hero took lies no more (#1256, #1296), nor an object that a `DelNPC`
    // removed (#2758).
    if npcs.taken.contains(&index) || npcs.removed.contains(&index) {
        return;
    }
    // A `CreateNPC` of the operand `54` armed once, by the rules of the `Enter` that ran last as
    // it ran; a crossbow its model cannot shoot another weapon at the hero's level
    // (**hypothesis**: `sdk_CREATENPC` passes a level held by the hero's, `0x4848AB`; #3072).
    if let Some(sector) = npcs.placements[index].by_rules.take() {
        let mut placement = npcs.placements[index].clone();
        let creature = placement.creature.0;
        let rules = npcs
            .rules
            .get(&sector)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let mut rng = spawning::Rng::of(placement.cell.0, placement.cell.1, index);
        let weapon = |kind, rng: &mut spawning::Rng| {
            armoury.and_then(|armoury| armoury.weapon(kind, hero, rng))
        };
        let shield =
            |rng: &mut spawning::Rng| armoury.and_then(|armoury| armoury.shield(hero, rng));
        let held = arm_by_rules(
            &mut placement,
            (rules, creature),
            (&npcs.pools, &npcs.gear, &npcs.makers.switches, &npcs.slots),
            (armoury.is_some(), weapon, shield),
            &mut rng,
        );
        bevy::log::debug!(
            "npcs: a {creature} of the scripts on {:?} takes {held:?} by {sector:?}'s rules",
            placement.cell
        );
        npcs.placements[index] = placement;
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
    // The creature maker's variant of its type (`0x480D68`, #1370), the same each time the same
    // record makes it.
    let base = turned.as_ref().unwrap_or(&npcs.placements[index]);
    let made_as = (!base.object).then(|| {
        let mut dice = spawning::Rng::of(base.cell.1, base.cell.0, index);
        spawning::made_variant(base.creature.0, &mut dice)
    });
    let turned = match made_as.filter(|&made| made != base.creature.0) {
        // As a pair of no flags turns it, its flags word kept.
        Some(made) => match npcs.makers.retype(
            base,
            quests::Turn {
                from: base.creature.0,
                to: made,
                flags: 0,
            },
            &mut npcs.slots,
        ) {
            Ok(Some(varied)) => Some(varied),
            Ok(None) => turned,
            Err(e) => {
                bevy::log::warn!("npcs: {e:#}");
                turned
            }
        },
        None => turned,
    };
    let placement = turned.as_ref().unwrap_or(&npcs.placements[index]);
    // The start's and the `Init`s' creatures persist; the `Enter`s' are made again.
    let kept = !placement.object && !placement.made.0;
    let again = if kept {
        npcs.kept.again(index)
    } else if !placement.object {
        entered_again(&mut npcs.entered, index)
    } else {
        Again::Anew
    };
    // Made anew, the level its record makes it at: the start's creature's as the game starts,
    // with the hero of its first level and its survival counter at 0; a sector's as its function
    // runs.
    let made_at = matches!(again, Again::Anew).then(|| {
        if placement.started {
            npcs.scripted_level(placement, fight::HERO_LEVEL, fight::Played::default())
        } else {
            npcs.scripted_level(placement, hero, time)
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
    // Made anew at a `DefPos` of a radius, it takes a cell of its own about it, its home there
    // (`0x474528`, `0x4833EF`; #2442).
    let drawn = matches!(again, Again::Anew)
        .then(|| spread_cell(placement, index, (heights, crowd), &npcs.drawn))
        .flatten();
    if let Some((x, y)) = drawn {
        npcs.drawn.insert((x, y));
    }
    let spread = drawn
        .and_then(|(x, y)| Some((u32::try_from(x).ok()?, u32::try_from(y).ok()?)))
        .filter(|&cell| cell != placement.cell)
        .map(|cell| Placement {
            cell,
            ..placement.clone()
        });
    let placement = spread.as_ref().unwrap_or(placement);
    let entity = match again {
        Again::Not => return,
        Again::Anew => {
            // A script's boss then at the hero's level now and more, as a fill's (`0x550E50`).
            let level = made_at.unwrap_or_default();
            let values = npcs.values.get(&at).copied().unwrap_or_default();
            let big = placement.creature.1 & crowd::BIG != 0;
            let level = placement
                .rank
                .level(level, (hero, time), &values, big, &npcs.fight);
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
    // A slave of Shaddar's lives while its sector is in the world, never made again
    // (docs/re/effects.md, "Shaddar's slaves"), and the big demon's illusion so, which ends of
    // itself (#3059).
    if placement.scripted.is_some_and(|(kind, _)| {
        kind == opensacred_rules::shaddar::SLAVE
            || kind == opensacred_rules::particles::BIG_DEMON_ILLUSION
    }) {
        npcs.removed.insert(index);
    }
    // A loaded save's queue of it, which it takes on again (#2890).
    if let Some(queue) = npcs.queues.remove(&index) {
        queue.give(commands, entity);
    }
    // An item lies as the rules make it, once, of its level kept (#1125).
    let laid = if placement.lies || placement.quest.is_some() {
        laid_item(
            (&mut npcs.laid, npcs.values.get(&at), &npcs.fight),
            (index, placement),
            (hero, made),
        )
    } else {
        plain_item(placement.creature.0)
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
                dropped: laid.clone(),
                resting: true,
            },
        ));
    }
    // An item of a `CreateObj` lies to be picked up as a drop (#1296).
    if placement.lies && placement.quest.is_none() {
        commands.entity(entity).insert((
            ScriptItem {
                index,
                uses: placement.uses.clone(),
                take: placement.taking.clone(),
            },
            // Its script name, by which the test `0x3A` and a `DelNPC` find it wherever the hero
            // puts it (#3147, #3217).
            drops::Lying {
                dropped: opensacred_rules::drops::Dropped {
                    script: placement.key.as_ref().map(|key| key.to_lowercase()),
                    ..laid.clone()
                },
                resting: true,
            },
        ));
    }
    // A special, which takes a label under Alt by its name or its type's (#1425).
    if placement.special {
        commands
            .entity(entity)
            .insert(Special(placement.creature.0));
    }
    // A lying item of a `res:` name goes by its text (#1360), and so does a creature, whose click
    // names it so (#1674), and a special, whose label shows it (#1425).
    if (placement.lies || placement.quest.is_some() || !placement.object || placement.special)
        && let Some(name) = placement.key.as_deref().and_then(TextName::of)
    {
        commands.entity(entity).insert(name);
    }
    if !placement.object {
        commands.entity(entity).insert(PlacementIndex(index));
    }
    // Its name in a save's references, an object's too (#3755).
    if let Ok(index) = u32::try_from(index) {
        commands.entity(entity).insert(Named(Id::Placed(index)));
    }
    // An object goes by its name for a `DelNPC` (#2758).
    if placement.object
        && let Some(name) = &placement.name
    {
        commands.entity(entity).insert(ObjectName(name.clone()));
    }
    // An `Enter`'s creature keeps what it was made of, which a `RegionChange` changes (#1064).
    if placement.made.0 && !placement.object {
        commands
            .entity(entity)
            .insert(Remakes(Box::new(placement.clone())));
    }
    // A door whose trigger is open, as it was left or as the scripts set it while it was
    // gone, opens as it comes (#952).
    if placement.door
        && placement
            .trigger
            .as_ref()
            .is_some_and(|trigger| npcs.opened.contains(trigger))
    {
        commands.entity(entity).insert(Swing::by_script(true));
    }
    // A container, emptied for good once opened (#1380).
    if let Some(function) = &placement.container {
        commands
            .entity(entity)
            .insert(crate::containers::Container::new(
                placement.creature.0,
                assets.load(placement.model.clone()),
                (function.clone(), placement.trigger.clone()),
                index,
                npcs.emptied.contains(&index),
            ));
    }
    if kept {
        let placed = Placed {
            at,
            source: Source::Kept(index),
        };
        npcs.kept.spawned(&placed);
        commands.entity(entity).insert(placed);
    }
    streamer.adopt(commands, at, entity);
    // A kept creature comes again with the weapon chosen for it (`respawn`).
    if kept
        && turned.is_none()
        && let Some(rearmed) = rearmed
    {
        npcs.placements[index] = rearmed;
    }
}

/// The cell that the creature or object of `placement`, made anew at a `DefPos` of a radius,
/// takes about it (`quests::Place::drawn`, the field reader's draw at `0x474528`, #2442): one that
/// may be walked on at its level with no creature standing in it nor made there in this frame
/// (`drawn`), as the original's test of a cell (`0x472AF0`) refuses the masses, the walls and the
/// cells that a creature holds, a cell of no loaded sector free; drawn by dice of its record, so
/// that the same record makes it at the same cell. None for a placement of no radius.
fn spread_cell(
    placement: &Placement,
    index: usize,
    (heights, crowd): (&Heights, Option<&crowd::Crowd>),
    drawn: &HashSet<(i32, i32)>,
) -> Option<(i32, i32)> {
    if placement.spread <= 0 {
        return None;
    }
    let (x, y) = placement.cell;
    let place = quests::Place {
        cell: (i32::try_from(x).ok()?, i32::try_from(y).ok()?),
        radius: placement.spread,
        level: placement.layer,
    };
    let floor = usize::try_from(placement.layer).ok().filter(|&f| f > 0);
    let level_of = |cell| {
        floor.and_then(|floor| {
            heights
                .building_on_floor(cell, floor)
                .and_then(|building| heights.floor_level(building, floor))
        })
    };
    let mut dice = spawning::Rng::of(x, y, index);
    Some(place.drawn(&mut dice, |cell| {
        let level = level_of(cell);
        !heights.holds(cell)
            || (heights.walkable_on(cell, level)
                && !drawn.contains(&cell)
                && crowd.is_none_or(|crowd| !crowd.crowded(cell, level, Entity::PLACEHOLDER)))
    }))
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
    (experience, difficulty): (Res<fight::Experience>, Option<Res<crate::game::Difficulty>>),
    (played, replacements, area_types): (
        Res<drops::Survival>,
        Option<Res<crate::scripts::ScriptReplacements>>,
        Option<Res<crate::scripts::ScriptAreaTypes>>,
    ),
    (items, settings): (
        Option<Res<drops::Tables>>,
        Option<Res<crate::settings::Settings>>,
    ),
    crowd: Option<Res<crowd::Crowd>>,
    bodies: Query<(&fight::Base, Option<&fight::Holding>), With<hero::Body>>,
) {
    let npcs = &mut *npcs;
    npcs.drawn.clear();
    let crowd = crowd.as_deref();
    let hero = experience.level;
    // The options' detail, at whose lowest the fills leave the animals out (`0x47CADD`).
    let detail = settings
        .as_deref()
        .map_or(2, |settings| settings.graphics.detail);
    let time = played.time(bodies.iter().next().and_then(|(_, holding)| holding));
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
        |(base, _)| drops::hero(base, hero),
    );
    let armoury = items.as_ref().map(|items| Armoury {
        catalogue: items.catalogue(),
        hero: &chooser,
    });
    let difficulty = difficulty.map(|d| d.0).unwrap_or_default();
    let made = items.as_deref().map(|items| (items, difficulty));
    // The placements that the play's scripts made in the sectors in the world (#1014).
    for index in std::mem::take(&mut npcs.pending) {
        let (x, y) = npcs.placements[index].cell;
        place_index(
            &mut commands,
            npcs,
            (index, (x / side, y / side)),
            (&heights, &states, &assets, crowd),
            (hero, time, area_types),
            (armoury, made),
            &mut streamer,
        );
    }
    let loaded: Vec<(u32, u32)> = streamer.sectors().collect();
    for &at in &loaded {
        if npcs.spawned.contains(&at) {
            continue;
        }
        npcs.spawned.insert(at);
        npcs.last_enter = Some(at);
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
                (&heights, &states, &assets, crowd),
                (hero, time, area_types),
                (armoury, made),
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
        let bound = if dungeons.sector(at) {
            npcs.respawns.dungeon
        } else {
            npcs.respawns.open
        };
        let entered = npcs.kept.enter(at, filling, f64::from(bound), |sector| {
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
        // A kept black horse's rider seated on it again, on the cell it shares with it
        // (`0x493E2B`, #2288); one whose horse comes back no more stands on foot.
        let horses: Vec<(u32, u32)> = entered
            .back
            .iter()
            .filter(|(placement, _)| placement.creature.0 == BLACK_HORSE)
            .map(|(_, stood)| stood.cell)
            .collect();
        let mut horses_back = Vec::new();
        let mut riders_back = Vec::new();
        for (mut placement, stood) in entered.back {
            if placement.rides && !horses.contains(&stood.cell) {
                on_foot(&mut placement, &npcs.slots);
            }
            let cell = stood.cell;
            let (horse, rider) = (placement.creature.0 == BLACK_HORSE, placement.rides);
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
            streamer.adopt(&mut commands, at, entity);
            if horse {
                horses_back.push((cell, entity));
            } else if rider {
                riders_back.push((cell, entity));
            }
        }
        for (cell, rider) in riders_back {
            if let Some(i) = horses_back.iter().position(|&(at, _)| at == cell) {
                let (_, horse) = horses_back.swap_remove(i);
                commands
                    .queue(move |world: &mut bevy::prelude::World| seat_rider(world, rider, horse));
            }
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
            let fill = &*spawning::at_detail(fill, detail);
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
            let shared = fight::fill_level(&values, hero, time, &npcs.fight, &mut rng);
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
                let weapon = |kind, rng: &mut spawning::Rng| {
                    armoury.and_then(|armoury| armoury.weapon(kind, shared, rng))
                };
                let shield = |rng: &mut spawning::Rng| {
                    armoury.and_then(|armoury| armoury.shield(shared, rng))
                };
                let held = arm_by_rules(
                    &mut placement,
                    (rules, made.creature),
                    (&npcs.pools, &npcs.gear, &npcs.makers.switches, &npcs.slots),
                    (armoury.is_some(), weapon, shield),
                    &mut rng,
                );
                if !held.is_empty() {
                    bevy::log::debug!(
                        "npcs: a {} of {at:?}'s fills on {:?} takes {held:?}",
                        made.creature,
                        placement.cell,
                    );
                }
                // A magician's spell of the rule (#2949).
                if let Some(spell) = spell_of(rules, made.creature, &mut rng) {
                    placement.spell = spell;
                }
                // The name its rules call it by (#3071).
                placement.called = called_of(rules, made.creature);
                // A random heading, as the creature maker gives one (`rand() % 361`).
                placement.heading = f32::from(u16::try_from(rng.below(361)).unwrap_or(0));
                // A hostile one bit 0, a peaceful one its type's behaviour (`0x480D40`), and a
                // critter its constructor's bit (`0x52DE40`), which no `CreateNPC` writes over;
                // the run's flags word that a `ReplaceSpawn` gave it OR'd in, bit 0 by its
                // hostility, a peaceful one of a word taking no behaviour of its type
                // (`0x480FBE`, `0x481048`; #3073).
                placement.creature.2 |= crowd::constructed(made.creature, placement.creature.1)
                    | match (made.hostile, fill.word) {
                        (true, word) => word | crowd::HOSTILE,
                        (false, 0) => ai::of_type(made.creature),
                        (false, word) => word & !crowd::HOSTILE,
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
                    placement
                        .rank
                        .level(1, (hero, time), &values, big, &npcs.fight)
                };
                // A black rider's horse, of the undead's side, brings its rider (`0x47E863`,
                // #2288).
                let black = made.creature == BLACK_HORSE;
                if black {
                    placement.side = UNDEAD_SIDE;
                }
                let entity = spawn(
                    &mut commands,
                    &placement,
                    &heights,
                    &states,
                    &assets,
                    (level, &npcs.fight, &npcs.items),
                );
                let cell = placement.cell;
                let heading = placement.heading;
                let placed = Placed {
                    at,
                    source: Source::Filled(Box::new(placement)),
                };
                npcs.kept.spawned(&placed);
                commands.entity(entity).insert(placed);
                streamer.adopt(&mut commands, at, entity);
                if black {
                    black_rider(
                        &mut commands,
                        (npcs, &mut streamer),
                        (at, cell, heading, entity),
                        (shared, &values, &mut rng),
                        (&heights, &states, &assets),
                    );
                }
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

/// Makes a fill's black horse's rider (`FillSector` at `0x47E863`–`0x47EFA5`, #2288): a 256 at
/// the horse's cell `cell` and heading, a hostile boss of the undead's side at the run's level
/// `shared` held as a boss's (`0x564D60(L, 1, 0)`), holding the magic short bow or the tribal
/// axe by a coin, made at its level on bronze's scale (the item maker's `0x5D5080`, as [`spawn`]
/// makes every held item, #2338), seated on the horse `horse` ([`seat_rider`]); no `MakeGroup`
/// items, spells or group.
fn black_rider(
    commands: &mut Commands,
    (npcs, streamer): (&mut Npcs, &mut Streamer),
    (at, cell, heading, horse): ((u32, u32), (u32, u32), f32, Entity),
    (shared, values, rng): (u32, &spawning::SpawnValues, &mut spawning::Rng),
    (heights, states, assets): (&Heights, &buildings::Buildings, &AssetServer),
) {
    let Some(mut rider) = npcs.templates.get(&BLACK_RIDER).cloned() else {
        return;
    };
    rider.cell = cell;
    rider.heading = heading;
    let coin = usize::try_from(rng.below(2)).unwrap_or(0);
    let weapon = BLACK_RIDER_WEAPONS[coin.min(1)];
    if let Some(gear) = npcs.gear.get(&weapon) {
        let categories = [(gear.category, gear.kind)];
        rider.holding = InHand::of([(weapon, gear.category, gear.kind)]);
        rider.equipment = gear
            .model
            .clone()
            .zip(hands(&categories).first().copied().flatten())
            .map(|(model, hand)| Held {
                item: weapon,
                model,
                texture: gear.texture.clone(),
                hand,
                mount: Mount::held(gear.category),
                slot: None,
            })
            .into_iter()
            .collect();
        rider.held = held_class(&categories);
        rider.ranged = ranged(&categories);
    }
    if let Some(slots) = npcs.slots.get(&rider.model) {
        rider.clips = ClipNames::riding(slots, rider.held);
    }
    // Hostile and a boss, its flags word `0x80001` alone, of the undead's side.
    rider.creature.2 = crowd::constructed(BLACK_RIDER, rider.creature.1) | crowd::HOSTILE;
    rider.side = UNDEAD_SIDE;
    rider.rank = fight::Rank::Boss;
    rider.rides = true;
    let big = rider.creature.1 & crowd::BIG != 0;
    let level = fight::Rank::Boss.held(shared, Some(values), BLACK_RIDER, big, &npcs.fight);
    let entity = spawn(
        commands,
        &rider,
        heights,
        states,
        assets,
        (level, &npcs.fight, &npcs.items),
    );
    let placed = Placed {
        at,
        source: Source::Filled(Box::new(rider)),
    };
    npcs.kept.spawned(&placed);
    commands.entity(entity).insert(placed);
    streamer.adopt(commands, at, entity);
    commands.queue(move |world: &mut bevy::prelude::World| seat_rider(world, entity, horse));
}

/// A rider's placement on foot: no rider's any more, its clips its feet's.
fn on_foot(placement: &mut Placement, slots: &HashMap<String, motions::Slots>) {
    placement.rides = false;
    if let Some(slots) = slots.get(&placement.model) {
        placement.clips = ClipNames::of(slots, placement.held);
    }
}

/// Seats the rider `rider` on the horse `horse` (`0x55C640` both ways, slot 18, `0x555300(18)`,
/// #2288): its model's `Bip01` on the horse's `horse_bone` (`riding::Seat`), at the horse's
/// place, its moves at the horse's speeds by its class's pace, and the horse out of the way
/// under it, carried ([`carry_riders`]).
pub(crate) fn seat_rider(world: &mut bevy::prelude::World, rider: Entity, horse: Entity) {
    let (Some(at), Some(kind), Some(creature)) = (
        world.get::<Transform>(horse).copied(),
        world.get::<crate::riding::Horse>(horse).copied(),
        world.get::<ai::Mind>(horse).map(|mind| mind.creature),
    ) else {
        return;
    };
    let Some(npcs) = world.get_resource::<Npcs>() else {
        return;
    };
    let pace = npcs.horse_row(kind.class).pace_factor();
    let speeds = npcs.speeds_of(creature).map(|s| f32::from(s) * pace);
    if let Ok(mut entity) = world.get_entity_mut(horse) {
        let way = entity.take::<crowd::InTheWay>();
        entity.insert((crate::riding::Ridden::by(rider, way), Steered(None)));
    }
    let Ok(mut entity) = world.get_entity_mut(rider) else {
        return;
    };
    if let Some(mut mind) = entity.get_mut::<ai::Mind>() {
        mind.thinks.speed = speeds[0];
        mind.thinks.run = speeds[1];
        mind.thinks.rides = Some(rider.index_u32() % 2 == 1);
    }
    if let Some(mut transform) = entity.get_mut::<Transform>() {
        transform.translation = at.translation;
        transform.rotation = at.rotation;
    }
    entity.insert(crate::riding::Seat::Riding {
        horse,
        speeds,
        offset: Vec3::ZERO,
    });
    bevy::log::debug!("npcs: {rider} rides {horse}");
}

/// Carries each NPC's horse under its rider (the rider's moves handed to its horse, `0x4F4770`),
/// its walk or its run playing as the rider moves; a rider that dies lets its horse go, and one
/// whose horse dies or is gone is thrown off at once (`0x53ECA0`, #2288): at the point by the
/// horse, made again on foot with its health, its clips its feet's and its speeds its own, its
/// record no rider's.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn carry_riders(
    mut commands: Commands,
    (npcs, mut streamer, heights, states, assets): (
        Res<Npcs>,
        ResMut<Streamer>,
        Res<Heights>,
        Res<buildings::Buildings>,
        Res<AssetServer>,
    ),
    riders: Query<
        (
            Entity,
            &Transform,
            &crate::riding::Seat,
            (Option<&Walk>, Has<Dies>, Has<GetsOff>),
            (&Placed, Option<&fight::Health>, &crowd::InTheWay),
        ),
        (Without<hero::Body>, Without<crate::riding::Horse>),
    >,
    mut horses: Query<
        (
            &mut Transform,
            &mut Steered,
            Option<&crate::riding::Ridden>,
            Has<Dies>,
        ),
        With<crate::riding::Horse>,
    >,
) {
    for (rider, at, seat, (walk, dying, gets_off), (placed, health, way)) in &riders {
        let Some(horse) = seat.ridden() else {
            continue;
        };
        let alive = matches!(horses.get(horse), Ok((.., false)));
        if dying {
            // Its horse let go, back in the way.
            if let Ok((_, _, Some(ridden), _)) = horses.get(horse) {
                let back = ridden.way().copied();
                let mut entity = commands.entity(horse);
                entity.remove::<(crate::riding::Ridden, Steered)>();
                if let Some(back) = back {
                    entity.insert(back);
                }
            }
            commands.entity(rider).remove::<crate::riding::Seat>();
            continue;
        }
        // Dead or lost, it throws its rider; a door in the rider's way gets it off, its horse
        // left standing (#2293).
        if !alive || gets_off {
            throw_rider(
                &mut commands,
                (&npcs, &mut streamer),
                (rider, at, placed, health, way),
                (
                    horse,
                    horses.get(horse).ok().and_then(|(_, _, ridden, _)| {
                        ridden.and_then(crate::riding::Ridden::way).copied()
                    }),
                ),
                (&heights, &states, &assets),
            );
            continue;
        }
        if let Ok((mut transform, mut steered, ..)) = horses.get_mut(horse) {
            transform.translation = at.translation;
            transform.rotation = at.rotation;
            let gait = walk.filter(|walk| walk.moved).map(|walk| walk.runs);
            if steered.0 != gait {
                steered.0 = gait;
            }
        }
    }
}

/// How far beside its horse a thrown rider stands, in world units (**estimate**: the original
/// takes `0x59A3E0`'s point by the horse).
const THROWN_BESIDE: f32 = 60.0;

/// Throws the rider `rider` off its dead or lost horse (`0x53ECA0`, #2288): its horse back in the
/// way where it lives, the rider made again on foot beside it with its health.
fn throw_rider(
    commands: &mut Commands,
    (npcs, streamer): (&Npcs, &mut Streamer),
    (rider, at, placed, health, way): (
        Entity,
        &Transform,
        &Placed,
        Option<&fight::Health>,
        &crowd::InTheWay,
    ),
    (horse, back): (Entity, Option<crowd::InTheWay>),
    (heights, states, assets): (&Heights, &buildings::Buildings, &AssetServer),
) {
    if let Ok(mut entity) = commands.get_entity(horse) {
        entity.remove::<(crate::riding::Ridden, Steered)>();
        if let Some(back) = back {
            entity.insert(back);
        }
    }
    let Source::Filled(placement) = &placed.source else {
        commands.entity(rider).remove::<crate::riding::Seat>();
        return;
    };
    let mut placement = (**placement).clone();
    on_foot(&mut placement, &npcs.slots);
    let beside = at.translation + at.rotation * Vec3::Z * THROWN_BESIDE;
    let spot = Transform::from_translation(beside);
    let Some(stood) = health.and_then(|health| {
        stood_of(
            (&spot, way.level),
            Some(health),
            (false, false),
            Some(heights),
        )
    }) else {
        commands.entity(rider).remove::<crate::riding::Seat>();
        return;
    };
    let entity = respawn(
        commands,
        &placement,
        stood,
        (heights, states, assets),
        (&npcs.fight, &npcs.items),
    );
    commands.entity(entity).insert(Placed {
        at: placed.at,
        source: Source::Filled(Box::new(placement)),
    });
    streamer.adopt(commands, placed.at, entity);
    // The rider made again leaves its place to its new self, as a `RegionChange`'s does.
    commands.entity(rider).insert(Remade).despawn();
    bevy::log::info!("npcs: {rider} is thrown off {horse}, on foot as {entity}");
}

/// The level of the crabs and fish, which no level's setter raises (`Placement::plain`).
const PLAIN_LEVEL: u32 = 1;
/// Their most health, the stats' own (seen in the original, #575).
const PLAIN_HEALTH: u32 = 100;

/// Spawns the crabs and fish of the waters of sector `at`, come in with no creatures kept
/// (`0x4939F0`, `spawning::swimmers`): of its liquid materials, its area and whether it is a
/// dungeon, the cells of the loaded sectors alone, each at level 1, peaceful, facing its way. A
/// crab, which no `Creature.pak` record describes, is made of the record that [`Npcs::read`] makes
/// its type (`opensacred_rules::fight::recordless`, #664); a type that the game has no template
/// of is left out.
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
        streamer.adopt(commands, at, entity);
    }
}

/// Spawns `placement`'s creature where it stood when it went with its sector, on that floor,
/// with the health and the level it had; a body that its dialogue holds lying there (#2457),
/// hired by none.
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
    there.heading = stood.heading;
    if stood.lies() {
        there.hire = None;
    }
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
    if stood.lies() {
        commands.entity(entity).insert(Dies::fallen());
    }
    if stood.marked {
        commands.entity(entity).insert(crate::undead::Marked);
    }
    entity
}

/// Notes what a placed creature leaves as it goes with its sector, or with the game (#329): a
/// kept one's place, floor and health, or where its body lies that its dialogue holds (#2457),
/// or that it was killed, and the death's functions it still has (#656); a fill's living one,
/// with its sector.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn keep(
    removed: On<Remove<Placed>>,
    creatures: Query<(
        &Placed,
        &Transform,
        (
            Option<&crowd::InTheWay>,
            Option<&crate::riding::Ridden>,
            Option<&Corpse>,
        ),
        Option<&fight::Health>,
        (Has<Dies>, Has<Talk>),
        Option<&OnDeath>,
        (Has<Remade>, Has<crate::undead::Marked>),
    )>,
    heights: Option<Res<Heights>>,
    npcs: Option<ResMut<Npcs>>,
) {
    let (
        Ok((placed, transform, (way, ridden, corpse), health, dying, on_death, (remade, marked))),
        Some(mut npcs),
    ) = (creatures.get(removed.entity), npcs)
    else {
        return;
    };
    let Some(level) = level_of(way, ridden, corpse) else {
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
    let stood = stood_of((transform, level), health, dying, heights.as_deref())
        .map(|stood| Stood { marked, ..stood });
    bevy::log::debug!(
        "npcs: {} of {:?} goes, {stood:?}",
        removed.entity,
        placed.at
    );
    npcs.kept.gone(placed, stood);
    // One that stood in another sector's cell, walked or teleported there, comes back with that
    // sector (#2453): the prince's company of quest 55 in the catacombs' hideout.
    if let (Source::Kept(index), Some(stood)) = (&placed.source, stood) {
        let side = u32::try_from(SIDE).unwrap_or(64);
        let sector = (stood.cell.0 / side, stood.cell.1 / side);
        let filed = npcs
            .by_sector
            .get(&sector)
            .is_some_and(|list| list.contains(index));
        if !filed {
            npcs.file_under(*index, sector);
        }
    }
}

/// The level a creature stands or lies on: its place's in the others' `way`, a ridden horse's kept
/// aside under its rider (#2288), or where its `corpse` fell, its place in the way gone.
fn level_of(
    way: Option<&crowd::InTheWay>,
    ridden: Option<&crate::riding::Ridden>,
    corpse: Option<&Corpse>,
) -> Option<Level> {
    way.or_else(|| ridden.and_then(crate::riding::Ridden::way))
        .map(|way| way.level)
        .or_else(|| corpse.map(|corpse| corpse.level))
}

/// Where a creature at `transform` on `level` stands as it goes, on its floor with its `health`;
/// none once killed or dying, but a dead one's body that its dialogue holds (`dying` with a
/// dialogue: the dying action's hold, `0x53ECA0`; #2457), which lies there with no health
/// ([`Stood::lies`]).
fn stood_of(
    (transform, level): (&Transform, Level),
    health: Option<&fight::Health>,
    (dying, held): (bool, bool),
    heights: Option<&Heights>,
) -> Option<Stood> {
    let (x, y) = paths::cell_of(transform.translation.x, transform.translation.z);
    let layer = heights
        .and_then(|heights| heights.floor(level))
        .map_or(0, |(_, floor)| i32::from(floor));
    let (Ok(x), Ok(y), Some(&health)) = (u32::try_from(x), u32::try_from(y), health) else {
        return None;
    };
    let health = match (dying, held) {
        (false, _) if health.current > 0 => health,
        (true, true) => fight::Health {
            current: 0,
            ..health
        },
        _ => return None,
    };
    Some(Stood {
        cell: (x, y),
        layer,
        heading: heading_of(transform.rotation),
        health,
        marked: false,
    })
}

/// What the sectors keep of their creatures as a save keeps it (#1959; the original's object
/// manager keeps every creature, alive and dead, `0x602300`): [`Kept`] with the creatures of the
/// sectors in the world gone as [`keep`] notes them going with their sectors, so that a load puts
/// them back as their sectors come in, but for the hero's followers, which go with the hero
/// ([`Following`]), the bodies that their dialogues hold lying where they fell (#2457); and the
/// placements of the living and lying kept ones as the play left them (a weapon chosen anew, the
/// death's functions left, a `RegionChange`'s type), by index; and the queues of the kept ones in
/// the world that the scripts' anim mode gave steps to take (#2890), which they take again as
/// their sectors bring them back.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Creatures {
    kept: Kept,
    placements: Vec<(usize, Placement)>,
    queues: Vec<(usize, SavedQueue)>,
    /// The `Enter` creatures of the sectors in the world, by their placements: where each stood,
    /// or none where it was killed (#3761), as the original's sectors of the bit `0x800` run no
    /// `Enter` at a load and their creatures come back from the save as they stood (docs/re/
    /// saves.md, "The file").
    entered: Vec<(usize, Option<Stood>)>,
}

/// The creatures as a save keeps them ([`Creatures`]); none without the NPCs.
pub fn creatures(world: &mut bevy::prelude::World) -> Option<Creatures> {
    let entered = entered_of(world);
    let mut standing = world.query_filtered::<(
        &Placed,
        (
            &Transform,
            Option<&crowd::InTheWay>,
            Option<&crate::riding::Ridden>,
            Option<&Corpse>,
        ),
        Option<&fight::Health>,
        (Has<Dies>, Has<Talk>),
        (Option<&OnDeath>, Option<&ScriptQueue>),
        (
            Has<crate::followers::Follower>,
            Has<crate::riding::OwnHorse>,
            Has<RisesAgain>,
            Has<crate::undead::Marked>,
        ),
    ), Without<Remade>>();
    let world = &*world;
    let npcs = world.get_resource::<Npcs>()?;
    let heights = world.get_resource::<Heights>();
    let now = world
        .get_resource::<Time<bevy::time::Real>>()
        .map_or(0.0, Time::<bevy::time::Real>::elapsed_secs_f64);
    let mut placements: HashMap<usize, Placement> = HashMap::new();
    let mut queues = Vec::new();
    let mut gone = Vec::new();
    for (
        placed,
        (at, way, ridden, corpse),
        health,
        dying,
        (on_death, queue),
        (follows, owned, rises, marked),
    ) in standing.iter(world)
    {
        let Some(level) = level_of(way, ridden, corpse) else {
            continue;
        };
        let stood =
            stood_of((at, level), health, dying, heights).map(|stood| Stood { marked, ..stood });
        // A kept one leaves the death's functions it still has, as [`keep`] notes them, and the
        // scripts' mark to rise again as a hero's strike may have cleared it or Sharifa taken it
        // (`fight::marked_after`, #3294).
        if let Source::Kept(index) = placed.source
            && let Some(placement) = npcs.placements.get(index)
            && stood.is_some()
        {
            let on_death = on_death.map(|on_death| on_death.0.clone());
            let placement = Placement {
                on_death: on_death.unwrap_or_default(),
                rises,
                ..placement.clone()
            };
            placements.insert(index, placement);
        }
        // A living follower goes with the hero ([`following`]), and so does its own horse
        // ([`mount_of`], #2301), its placement kept out; a follower's body lies where it fell,
        // as quest 61's Shareefa (#2457).
        if (follows || owned) && stood.is_some_and(|stood| !stood.lies()) {
            continue;
        }
        // Its queue's steps, which it takes again as it comes back (#2890).
        if let (Source::Kept(index), Some(queue), Some(_)) = (&placed.source, queue, stood) {
            let saved = queue.saved(now);
            if !saved.is_empty() {
                queues.push((*index, saved));
            }
        }
        gone.push((placed, stood));
    }
    queues.sort_by_key(|&(index, _)| index);
    let mut kept = npcs.kept.with_gone(gone);
    if let Some(streamer) = world.get_resource::<Streamer>() {
        kept.standing = streamer.sectors().collect();
    }
    // The placements of those that went living or lying before; a killed one's comes back no
    // more.
    for (&index, went) in &kept.went {
        if let (Some(_), Some(placement)) = (went, npcs.placements.get(index)) {
            placements.entry(index).or_insert_with(|| placement.clone());
        }
    }
    let mut placements: Vec<(usize, Placement)> = placements.into_iter().collect();
    placements.sort_by_key(|&(index, _)| index);
    Some(Creatures {
        kept,
        placements,
        queues,
        entered,
    })
}

/// The `Enter` creatures of the sectors in `world`, by their placements, as a save keeps them
/// (#3761): where each living one stands, or its body lies that its dialogue holds, and none for
/// each that is not in the world, killed; a follower goes with the hero.
fn entered_of(world: &mut bevy::prelude::World) -> Vec<(usize, Option<Stood>)> {
    let mut made = world.query_filtered::<(
        &PlacementIndex,
        (
            &Transform,
            Option<&crowd::InTheWay>,
            Option<&crate::riding::Ridden>,
            Option<&Corpse>,
        ),
        Option<&fight::Health>,
        (Has<Dies>, Has<Talk>, Has<crate::undead::Marked>),
    ), (Without<Placed>, Without<crate::followers::Follower>)>();
    let world = &*world;
    let (Some(npcs), Some(streamer)) = (
        world.get_resource::<Npcs>(),
        world.get_resource::<Streamer>(),
    ) else {
        return Vec::new();
    };
    let heights = world.get_resource::<Heights>();
    let standing: HashMap<usize, Option<Stood>> = made
        .iter(world)
        .filter(|(index, ..)| {
            npcs.placements
                .get(index.0)
                .is_some_and(|placement| placement.made.0 && !placement.object)
        })
        .map(
            |(index, (at, way, ridden, corpse), health, (dying, held, marked))| {
                let stood = level_of(way, ridden, corpse)
                    .and_then(|level| stood_of((at, level), health, (dying, held), heights))
                    .map(|stood| Stood { marked, ..stood });
                (index.0, stood)
            },
        )
        .collect();
    let mut entered: Vec<(usize, Option<Stood>)> = streamer
        .sectors()
        .flat_map(|at| npcs.by_sector.get(&at).into_iter().flatten().copied())
        .filter(|&index| {
            npcs.placements
                .get(index)
                .is_some_and(|placement| placement.made.0 && !placement.object)
        })
        .map(|index| (index, standing.get(&index).copied().flatten()))
        .collect();
    entered.sort_by_key(|&(index, _)| index);
    entered.dedup_by_key(|&mut (index, _)| index);
    entered
}

/// What becomes of the `Enter` creature of the placement `index` as its sector comes in, a
/// loaded save's first coming taking where it stood or that it was killed, once (#3761).
fn entered_again(entered: &mut HashMap<usize, Option<Stood>>, index: usize) -> Again {
    match entered.remove(&index) {
        None => Again::Anew,
        Some(Some(stood)) => Again::AtStood(stood),
        Some(None) => Again::Not,
    }
}

/// A follower of the hero as a save keeps it (#1959): what it is made of, where it stands on which
/// floor with its health, its flags word, side and stand in the others' way as its hiring and its
/// summoning left them, whether it is of the flag 4, its hands' items with their values, the items
/// the hero gave it (`mercenaries`), and its effects; whether the scripts keep it to rise again
/// ([`RisesAgain`]), and where it lies dead to rise, how long it has lain, its health its most
/// (#2947); and the steps of its queue under the scripts' anim mode (#2890).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, MapEntities)]
pub struct Following {
    made: Made,
    stood: Stood,
    flags: u32,
    side: u8,
    stands: ways::InTheWay,
    escorted: bool,
    listed: bool,
    hands: Vec<(u32, Hand, Option<fight::ItemValues>)>,
    given: Option<crate::mercenaries::Given>,
    /// Its effects, their sources by their names (#3755).
    #[entities]
    effects: Option<crate::effects::Effects>,
    rises: bool,
    lain: Option<f32>,
    queue: SavedQueue,
}

impl Following {
    /// Takes the sources of its effects that a load found no creature for for none
    /// (`effects::Effects::forget_lost`).
    pub fn forget_lost(&mut self) {
        if let Some(effects) = &mut self.effects {
            effects.forget_lost();
        }
    }
}

/// What a saved follower is made of: a kept placement of its sector, a fill's placement of its
/// sector, or a creature of its type and rank that no sector keeps (a summoned, called or raised
/// one, `followers`).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
enum Made {
    Kept((u32, u32), usize),
    Filled((u32, u32), Box<Placement>),
    Roams {
        creature: u32,
        rank: fight::Rank,
        /// Its serial (`names::Id::Born`, #3755).
        born: Option<u32>,
    },
}

/// What a save keeps of a follower beside its place, mind and stand ([`Following`]): its
/// placement, its rank, its arms, the items the hero gave it, its effects, whether it rises
/// again, and how long it has lain dead to rise.
type FollowerParts<'a> = (
    (Option<&'a Placed>, Option<&'a Named>),
    Option<&'a fight::Ranked>,
    &'a Arms,
    (
        Option<&'a crate::mercenaries::Given>,
        Option<&'a crate::effects::Effects>,
    ),
    bool,
    Option<f32>,
);

/// A follower as a save keeps it: of the flag 4 where `escorted`, with an entry in the column
/// where `listed`, standing `stood`, of the mind `mind` and the stand `stands`, and what
/// [`FollowerParts`] adds.
fn followed(
    (escorted, listed): (bool, bool),
    stood: Stood,
    (mind, stands): (&ai::Mind, ways::InTheWay),
    (placed, ranked, arms, (given, effects), rises, lain): FollowerParts<'_>,
) -> Following {
    Following {
        made: made_of(placed, mind, ranked),
        stood,
        flags: mind.flags,
        side: mind.side,
        stands,
        escorted,
        listed,
        hands: arms
            .holding
            .iter()
            .map(|(held, values)| (held.item, held.hand, *values))
            .collect(),
        given: given.cloned(),
        effects: effects.cloned(),
        rises,
        lain,
        queue: SavedQueue::default(),
    }
}

/// The hero's followers as a save keeps them ([`Following`]), in the order of their entries in
/// the portraits' column: the living, and those that lie dead to rise again with how long they
/// have lain (#2947).
pub fn following(world: &mut bevy::prelude::World) -> Vec<Following> {
    let mut heroes = world.query_filtered::<Entity, With<hero::Body>>();
    let Some(hero) = heroes.iter(world).next() else {
        return Vec::new();
    };
    let mut followers = world.query_filtered::<(
        (
            Entity,
            &crate::followers::Follower,
            (Option<&Placed>, Option<&Named>),
        ),
        (&Transform, &crowd::InTheWay, Option<&fight::Health>),
        (&ai::Mind, &Arms, Option<&fight::Ranked>),
        (
            Option<&crate::mercenaries::Given>,
            Option<&crate::effects::Effects>,
        ),
        (Has<RisesAgain>, Option<&ScriptQueue>),
    ), Without<Dies>>();
    let mut lying = world.query_filtered::<(
        (
            Entity,
            &crate::followers::Follower,
            (Option<&Placed>, Option<&Named>),
        ),
        (&Transform, &Stashed, Option<&fight::Health>),
        (&Arms, Option<&fight::Ranked>),
        (
            Option<&crate::mercenaries::Given>,
            Option<&crate::effects::Effects>,
        ),
    ), (With<Dies>, With<RisesAgain>)>();
    let world = &*world;
    let heights = world.get_resource::<Heights>();
    let now = world
        .get_resource::<Time<bevy::time::Real>>()
        .map_or(0.0, Time::<bevy::time::Real>::elapsed_secs_f64);
    let order = world
        .get_resource::<crate::followers::List>()
        .map(|list| list.0.clone())
        .unwrap_or_default();
    let entry = |entity: Entity| {
        order
            .iter()
            .position(|&shown| shown == entity)
            .unwrap_or(usize::MAX)
    };
    let living = followers
        .iter(world)
        .filter(|((_, follower, _), ..)| follower.master == hero)
        .filter_map(
            |(
                (entity, follower, placed),
                (transform, way, health),
                (mind, arms, ranked),
                extra,
                (rises, queue),
            )| {
                let stood = stood_of((transform, way.level), health, (false, false), heights)?;
                let kept = (placed, ranked, arms, extra, rises, None);
                let mut following = followed(
                    (follower.escorted(), follower.listed()),
                    stood,
                    (mind, way.stands),
                    kept,
                );
                if let Some(queue) = queue {
                    following.queue = queue.saved(now);
                }
                Some((entry(entity), following))
            },
        );
    // A dead one kept to rise, at its most health, the time it has lain beside it.
    let dead = lying
        .iter(world)
        .filter(|((_, follower, _), ..)| follower.master == hero)
        .filter_map(
            |((entity, follower, placed), (transform, stashed, health), (arms, ranked), extra)| {
                let most = health.map(|health| fight::Health {
                    current: health.most,
                    ..*health
                });
                let level = stashed.way.level;
                let stood = stood_of((transform, level), most.as_ref(), (false, false), heights)?;
                let kept = (placed, ranked, arms, extra, true, Some(stashed.lain));
                let mind = (&stashed.mind, stashed.way.stands);
                Some((
                    entry(entity),
                    followed((follower.escorted(), follower.listed()), stood, mind, kept),
                ))
            },
        );
    let mut kept: Vec<(usize, Following)> = living.chain(dead).collect();
    kept.sort_by_key(|&(entry, _)| entry);
    kept.into_iter().map(|(_, following)| following).collect()
}

/// A dead creature's body as a save keeps it (#3762): what it is made of, where it lies with its
/// heading and level, what it was, how it died, how long it has lain and lies on before it
/// vanishes and is destroyed (`bodies`), and an undead's rising or burning (`undead`); a load
/// lays it where it lay at once, its fall not played again, its times going on
/// (docs/decisions/fight.md, "Bodies lie at a load as they lay").
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedBody {
    made: Made,
    stood: Stood,
    corpse: Corpse,
    choice: u32,
    by_creature: bool,
    by_follower: bool,
    killed: bool,
    rises: bool,
    lies: Option<crate::bodies::SavedLies>,
    undead: Option<crate::undead::SavedUndead>,
}

/// The dead creatures' bodies in `world` as a save keeps them (#3762): every named one that lies
/// or dies, but a follower's, which goes with the hero ([`following`]), and a kept placement's
/// whose dialogue holds it, which its sector keeps ([`Kept`], #2457); a dying one lies as its
/// fall will end.
pub fn bodies(world: &mut bevy::prelude::World) -> Vec<SavedBody> {
    let mut dead = world.query_filtered::<(
        Entity,
        (Option<&Placed>, Option<&crate::names::Named>),
        (&Transform, Option<&Corpse>, Option<&Stashed>),
        (&Dies, Option<&fight::Health>, Option<&fight::Ranked>),
        (Option<&ai::Mind>, Option<&crowd::InTheWay>, Has<Talk>),
    ), Without<crate::followers::Follower>>();
    let world = &*world;
    let heights = world.get_resource::<Heights>();
    let mut saved = Vec::new();
    for (
        entity,
        (placed, named),
        (at, corpse, stashed),
        (dies, health, ranked),
        (mind, way, talk),
    ) in dead.iter(world)
    {
        if named.is_none() || (talk && placed.is_some_and(|placed| placed.kept().is_some())) {
            continue;
        }
        let Some(mind) = stashed.map(|stashed| &stashed.mind).or(mind) else {
            continue;
        };
        let level = corpse
            .map(|corpse| corpse.level)
            .or_else(|| stashed.map(|stashed| stashed.way.level))
            .or_else(|| way.map(|way| way.level))
            .unwrap_or_default();
        let health = health.map(|health| fight::Health {
            current: 0,
            ..*health
        });
        let Some(stood) = stood_of((at, level), health.as_ref(), (true, true), heights) else {
            continue;
        };
        let corpse = corpse.copied().unwrap_or(Corpse {
            creature: mind.creature,
            class: mind.class,
            dialogue: mind.dialogue,
            level,
            hostile: mind.hostile(),
            flags: mind.flags,
        });
        saved.push(SavedBody {
            made: made_of((placed, named), mind, ranked),
            stood,
            corpse,
            choice: dies.choice,
            by_creature: dies.by_creature,
            by_follower: dies.by_follower,
            killed: dies.killed,
            rises: dies.rises,
            lies: crate::bodies::saved_lies(world, entity),
            undead: crate::undead::saved(world, entity),
        });
    }
    saved
}

/// Lays a loaded save's bodies again where they lay (#3762), each made as it was, its fall not
/// played again, its times of lying and its undead's state going on.
pub fn lay_again(world: &mut bevy::prelude::World, bodies: Vec<SavedBody>) {
    let count = bodies.len();
    for body in bodies {
        let following = Following {
            made: body.made,
            stood: body.stood,
            flags: body.corpse.flags,
            side: 0,
            stands: ways::InTheWay::object(),
            escorted: false,
            listed: false,
            hands: Vec::new(),
            given: None,
            effects: None,
            rises: false,
            lain: None,
            queue: SavedQueue::default(),
        };
        let entity = match world.run_system_cached_with(respawn_following, following) {
            Ok(Ok(entity)) => entity,
            Ok(Err(e)) => {
                bevy::log::warn!("npcs: a saved body is not made: {e}");
                continue;
            }
            Err(e) => {
                bevy::log::warn!("npcs: a saved body is not made: {e}");
                continue;
            }
        };
        let Ok(mut creature) = world.get_entity_mut(entity) else {
            continue;
        };
        creature.insert(Dies {
            choice: body.choice,
            by_creature: body.by_creature,
            by_follower: body.by_follower,
            killed: body.killed,
            rises: body.rises,
            ..Dies::fallen()
        });
        if let Some(lies) = body.lies {
            creature.insert(crate::bodies::Resumed(lies));
        }
        if let Some(undead) = body.undead {
            crate::undead::again(&mut creature, undead);
        }
    }
    bevy::log::info!("npcs: the save's {count} bodies lie again");
}

/// What a saved creature is made of ([`Made`]) by its placement `placed`, else its type, rank and
/// serial of its name `named`.
fn made_of(
    (placed, named): (Option<&Placed>, Option<&Named>),
    mind: &ai::Mind,
    ranked: Option<&fight::Ranked>,
) -> Made {
    match placed {
        Some(Placed {
            at,
            source: Source::Kept(index),
        }) => Made::Kept(*at, *index),
        Some(Placed {
            at,
            source: Source::Filled(placement),
        }) => Made::Filled(*at, placement.clone()),
        None => Made::Roams {
            creature: mind.creature,
            rank: ranked.map_or(fight::Rank::Common, |ranked| ranked.0),
            born: named.and_then(|named| match named.0 {
                Id::Born(serial) => Some(serial),
                _ => None,
            }),
        },
    }
}

/// The hero's own horse as a save keeps it (#2301; the original's save keeps it with its
/// objects, `0x602300`): as a follower ([`Following`]), what it is made of and where it stands
/// with its health and effects, under its rider where the hero rides it, its place in the way
/// kept aside (#3743); none where the hero owns none, or one dead.
pub fn mount_of(world: &mut bevy::prelude::World) -> Option<Following> {
    let mut horses = world.query_filtered::<(
        (Option<&Placed>, Option<&Named>),
        (
            &Transform,
            Option<&crowd::InTheWay>,
            Option<&crate::riding::Ridden>,
            Option<&fight::Health>,
        ),
        (&ai::Mind, Option<&fight::Ranked>, Option<&Arms>),
        Option<&crate::effects::Effects>,
    ), (With<crate::riding::OwnHorse>, Without<Dies>)>();
    let world = &*world;
    let heights = world.get_resource::<Heights>();
    let (placed, (transform, way, ridden, health), (mind, ranked, arms), effects) =
        horses.iter(world).next()?;
    let way = way.or_else(|| ridden.and_then(crate::riding::Ridden::way))?;
    let stood = stood_of((transform, way.level), health, (false, false), heights)?;
    bevy::log::debug!("npcs: the hero's horse saved standing {stood:?}");
    Some(Following {
        made: made_of(placed, mind, ranked),
        stood,
        flags: mind.flags,
        side: mind.side,
        stands: way.stands,
        escorted: false,
        listed: false,
        hands: arms.map_or_else(Vec::new, |arms| {
            arms.holding
                .iter()
                .map(|(held, values)| (held.item, held.hand, *values))
                .collect()
        }),
        given: None,
        effects: effects.cloned(),
        rises: false,
        lain: None,
        queue: SavedQueue::default(),
    })
}

/// Puts the saved own horse `horse` back where it stood and links it to the hero again: the
/// horse made again, which a hero that rode it mounts again once it has landed (`saves`, #3743),
/// as the original's load of a game keeps both creatures' records and their link (docs/re/saves.md,
/// "What a load puts back").
pub fn mount_again(world: &mut bevy::prelude::World, horse: Following) -> Option<Entity> {
    let (flags, side, stands) = (horse.flags, horse.side, horse.stands);
    let entity = match world.run_system_cached_with(respawn_following, horse) {
        Ok(Ok(entity)) => entity,
        Ok(Err(e)) => {
            bevy::log::warn!("npcs: the saved horse is not made: {e}");
            return None;
        }
        Err(e) => {
            bevy::log::warn!("npcs: the saved horse is not made: {e}");
            return None;
        }
    };
    if let Some(mut mind) = world.get_mut::<ai::Mind>(entity) {
        mind.flags = flags;
        mind.side = side;
    }
    if let Some(mut way) = world.get_mut::<crowd::InTheWay>(entity) {
        way.stands = stands;
    }
    crate::riding::own(world, entity);
    let at = world
        .get::<Transform>(entity)
        .map(|t| paths::cell_of(t.translation.x, t.translation.z));
    bevy::log::info!("npcs: the save's horse {entity} back at {at:?}, the hero's own");
    Some(entity)
}

/// What a follower made of a placement whose hands hold `made` changes to hold `hands` again
/// (#1959): each hand whose item differs emptied, and the item it held taken with its values;
/// none where they are the same.
fn rearms_for(hands: &[(u32, Hand, Option<fight::ItemValues>)], made: &[InHand]) -> Option<Rearms> {
    let mut rearms = Rearms {
        away: Vec::new(),
        takes: Vec::new(),
        disarmed: false,
        wears: Vec::new(),
    };
    for hand in [Hand::Left, Hand::Right] {
        let held = hands.iter().find(|&&(_, held, _)| held == hand);
        let item = made
            .iter()
            .find(|held| held.hand == hand)
            .map(|held| held.item);
        if held.map(|&(item, ..)| item) == item {
            continue;
        }
        rearms.away.push(hand);
        rearms.takes.extend(held.copied());
    }
    (!rearms.away.is_empty()).then_some(rearms)
}

/// Spawns a saved follower where it stood ([`follow_again`]), as a sector puts a creature back
/// ([`respawn`]): of its kept placement, its fill's or its type's, with its hands' items, the
/// items given and its effects, going with the game alone as a hired one does.
fn respawn_following(
    In(following): In<Following>,
    mut commands: Commands,
    mut npcs: ResMut<Npcs>,
    heights: Res<Heights>,
    states: Res<buildings::Buildings>,
    assets: Res<AssetServer>,
    data: Res<crate::game::Data>,
) -> Result<Entity, String> {
    let npcs = &mut *npcs;
    let Following {
        made,
        stood,
        hands,
        given,
        effects,
        ..
    } = following;
    let mut roams = None;
    let (placement, placed) = match made {
        Made::Kept(at, index) => {
            let placement = npcs
                .placements
                .get(index)
                .cloned()
                .ok_or_else(|| format!("no placement {index}"))?;
            let source = Source::Kept(index);
            (placement, Some(Placed { at, source }))
        }
        Made::Filled(at, placement) => {
            let made = (*placement).clone();
            let source = Source::Filled(placement);
            (made, Some(Placed { at, source }))
        }
        Made::Roams {
            creature,
            rank,
            born,
        } => {
            roams = born;
            let request = Spawn {
                creature,
                cells: Vec::new(),
                floor: stood.layer,
                level: None,
                rank,
                hostile: Some(false),
                immortal: false,
                roams: true,
            };
            (npcs.requested(&request, &data.0.assets)?, None)
        }
    };
    let entity = respawn(
        &mut commands,
        &placement,
        stood,
        (&heights, &states, &assets),
        (&npcs.fight, &npcs.items),
    );
    let mut creature = commands.entity(entity);
    creature.insert(bevy::prelude::DespawnOnExit(crate::game::AppState::InGame));
    if let Some(rearms) = rearms_for(&hands, &placement.holding) {
        creature.insert(rearms);
    }
    if let Some(given) = given {
        creature.insert(given);
    }
    if let Some(effects) = effects {
        creature.insert(effects);
    }
    if let Some(placed) = placed {
        npcs.kept.spawned(&placed);
        creature.insert(placed);
    }
    if let Some(serial) = roams {
        creature.insert(Named(Id::Born(serial)));
    }
    Ok(entity)
}

/// Names a placed creature as it is placed (#3755): a kept placement's by its index, a fill's or
/// a summoned one's by the serial its placement carries, one given here where it carries none
/// yet, which the placement keeps as its sector keeps it.
fn named(
    added: On<bevy::prelude::Add<Placed>>,
    mut commands: Commands,
    mut placed: Query<&mut Placed>,
    npcs: Option<ResMut<Npcs>>,
) {
    let (Ok(mut placed), Some(mut npcs)) = (placed.get_mut(added.entity), npcs) else {
        return;
    };
    let id = match &mut placed.source {
        Source::Kept(index) => match u32::try_from(*index) {
            Ok(index) => Id::Placed(index),
            Err(_) => return,
        },
        Source::Filled(placement) => {
            Id::Born(*placement.born.get_or_insert_with(|| npcs.kept.mint()))
        }
    };
    commands.entity(added.entity).insert(Named(id));
}

/// Puts a loaded save's followers back beside the hero once it stands (#1959): each made as it was
/// ([`Following`]) where it stood, hired by the hero again with the flags word, side and stand in
/// the others' way that its hiring left it, not by its placement's hire again
/// (`followers::take_hires`); its kept placement stays out of its sector ([`Npcs::remember`]).
/// One kept to rise again keeps it ([`RisesAgain`]), and one that lay dead to rise lies down again
/// at once, to rise once it has lain its whole time (`followers::rise_again`, #2947).
pub fn follow_again(world: &mut bevy::prelude::World, followers: Vec<Following>) {
    let mut heroes = world.query_filtered::<Entity, With<hero::Body>>();
    let Some(master) = heroes.iter(world).next() else {
        return;
    };
    let count = followers.len();
    let mut hired = Vec::new();
    for following in followers {
        let (flags, side, stands, escorted, listed) = (
            following.flags,
            following.side,
            following.stands,
            following.escorted,
            following.listed,
        );
        let (rises, lain, queue) = (following.rises, following.lain, following.queue.clone());
        let entity = match world.run_system_cached_with(respawn_following, following) {
            Ok(Ok(entity)) => entity,
            Ok(Err(e)) => {
                bevy::log::warn!("npcs: a saved follower is not made: {e}");
                continue;
            }
            Err(e) => {
                bevy::log::warn!("npcs: a saved follower is not made: {e}");
                continue;
            }
        };
        let Ok(mut creature) = world.get_entity_mut(entity) else {
            continue;
        };
        creature.remove::<(crate::followers::Hired, fight::Unfought)>();
        crate::followers::rehire(world, entity, master, (escorted, listed));
        hired.push(entity);
        if let Some(mut mind) = world.get_mut::<ai::Mind>(entity) {
            mind.flags = flags;
            mind.side = side;
        }
        if let Some(mut way) = world.get_mut::<crowd::InTheWay>(entity) {
            way.stands = stands;
        }
        let Ok(mut creature) = world.get_entity_mut(entity) else {
            continue;
        };
        if rises {
            creature.insert(RisesAgain);
        } else {
            creature.remove::<RisesAgain>();
        }
        if let Some(lain) = lain {
            if let Some(mut health) = creature.get_mut::<fight::Health>() {
                health.current = 0;
            }
            creature.insert(Dies::lying(lain));
            bevy::log::info!("npcs: a saved follower lies again, {lain:.1} s lain");
        }
        queue.give_now(world, entity);
    }
    // The hero's list in the save's order (`followers::List`, #3102).
    if let Some(mut list) = world.get_resource_mut::<crate::followers::List>() {
        list.0 = hired;
    }
    bevy::log::info!("npcs: the save's {count} followers back beside the hero");
}

/// The point `height` units of the models' height over the point `spot` of the ground at height
/// 0, as the original puts an object of a script's height (the record's tag `0x7E`): the record's
/// point taken into its models' space on the plane of height 0, the height its third coordinate
/// (`0x489022`, `0x6239C0`, `0x623940`), which its view raises by
/// [`iso::PIXELS_PER_MODEL_HEIGHT`] pixels a unit, with no ground under it added (`0x5A5AE2`). The
/// models' scale's 1.66 pixels a unit, and the point moved along the line of sight to the
/// standing height of its cell as a model's is, left a wall torch's flame over stairs below its
/// bowl (#2728).
fn raised(spot: Vec2, height: f32) -> Vec3 {
    Vec3::new(
        spot.x,
        iso::height(height * iso::PIXELS_PER_MODEL_HEIGHT),
        spot.y,
    )
}

/// Spawns the NPC or the object of `placement`: its model on its cell and level, an object's at
/// its drawn position (`model_position`), turned to its heading, with its texture, its items, its
/// shadow, its clips, its walk and its place in the others' way. An object that has no model (an
/// effect, a piece of the Pong, a summoning, an element of the forge, a dwarves' magic, a prison,
/// a sound's loop) is spawned as its own component at its point instead.
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
    // A creature put on the ground on a building's mass goes onto the building's floor 1 where
    // its grid holds the cell walkable, as the object manager's move puts it (`0x5FCCB0`,
    // docs/re/movement.md, "Changing layers"), and a walking one ([`walk`]): the garden's
    // novice of quest 15001 in the monastery, whom the hero's way had sought on the ground
    // within the walls (#1735).
    let massed = match (i32::try_from(x), i32::try_from(y), floor) {
        (Ok(cx), Ok(cy), None) if !placement.object && !heights.walkable((cx, cy)) => {
            heights.level_of((cx, cy))
        }
        _ => None,
    };
    let level = match (on_floor, floor) {
        (Some(building), Some(floor)) => heights.floor_level(building, floor),
        _ => massed,
    };
    position.y = heights
        .standing(position.x, position.z, level)
        .unwrap_or(0.0);
    // In its layer of the building whose floor or cell it is on, if any.
    let layered = if massed.is_some() {
        buildings::Layered::at(heights, position.x, position.z, level)
    } else {
        buildings::Layered::of(
            heights,
            on_floor.or_else(|| {
                heights
                    .building(position.x, position.z)
                    .map(|(building, _)| building)
            }),
            placement.layer,
        )
    };
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
    // A placed object's point on the plane of height 0: its record's, else its cell's centre.
    let spot = placement
        .drawn
        .unwrap_or_else(|| Vec2::new(position.x, position.z));
    if let Some((variant, height, over)) = placement.fx {
        return crate::fx::spawn(
            commands,
            (variant, level, over),
            raised(spot, height),
            (layered, layered.visibility(states)),
        );
    }
    if let Some((piece, height)) = placement.pong {
        return commands
            .spawn((
                piece,
                Transform::from_translation(raised(spot, height)),
                layered,
                layered.visibility(states),
            ))
            .id();
    }
    if let Some((variant, height)) = placement.summon {
        return commands
            .spawn((
                crate::summonings::Summon(variant),
                Transform::from_translation(raised(spot, height)),
                layered,
                layered.visibility(states),
            ))
            .id();
    }
    if let Some((variant, height)) = placement.forge {
        return commands
            .spawn((
                crate::forge_elements::Element {
                    variant,
                    name: placement.key.clone(),
                },
                Transform::from_translation(raised(spot, height)),
                layered,
                layered.visibility(states),
            ))
            .id();
    }
    if let Some((kind, height)) = placement.scripted {
        let mut made = commands.spawn((
            Transform::from_translation(raised(spot, height)),
            layered,
            layered.visibility(states),
        ));
        if kind == script_traps::FIELD {
            made.insert(crate::pill_fields::Field);
        } else if kind == script_traps::FENCES {
            made.insert(crate::laser_fences::Maker);
        } else if kind == opensacred_rules::shaddar::SLAVE {
            made.insert(crate::shaddar_slaves::Slave);
        } else if kind == opensacred_rules::zhadum::ZHADUM {
            made.insert(crate::zhadums::Zhadum);
        } else if kind == opensacred_rules::particles::BIG_DEMON_ILLUSION {
            made.insert(crate::demon_illusions::Illusion);
        } else if crate::underworld_glyphs::Underworld::is(kind) {
            made.insert(crate::underworld_glyphs::Underworld {
                kind,
                name: placement.key.clone(),
            });
        } else {
            made.insert(crate::hell_gates::Gate {
                kind,
                name: placement.key.clone(),
            });
        }
        return made.id();
    }
    if let Some((variant, height)) = placement.dwarf_magic {
        return commands
            .spawn((
                crate::dwarf_magics::Magic(variant),
                Transform::from_translation(raised(spot, height)),
                layered,
                layered.visibility(states),
            ))
            .id();
    }
    if let Some((kind, height)) = placement.prison {
        let mut made = commands.spawn((
            Transform::from_translation(raised(spot, height)),
            layered,
            layered.visibility(states),
        ));
        if kind == BUBBLE_PRISON {
            made.insert(crate::bubble_prisons::Bubble);
        } else {
            made.insert(crate::prisons::Prison);
        }
        return made.id();
    }
    if let Some((sound, kind)) = placement.fx_loop {
        let mut made = commands.spawn((
            crate::fx_loops::FxLoop(sound),
            Transform::from_translation(model),
            layered,
            layered.visibility(states),
        ));
        if let Some(gate) = crate::stargates::Gate::of(kind) {
            made.insert(gate);
        }
        if let Some(stone) = crate::element_stones::Stone::of(kind) {
            made.insert(stone);
        }
        return made.id();
    }
    let mut entity = commands.spawn((
        models::scene(assets, &placement.model),
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
    // A vision, of its type or a script's mark (#2519, #3064).
    if !placement.object
        && (placement.vision || opensacred_rules::spells::VISIONS.contains(&placement.creature.0))
    {
        envision(entity.reborrow(), placement.creature.0);
    }
    if placement.casts {
        entity.insert(shadows::Casts);
    }
    if placement.object {
        entity.insert(Object {
            at: position,
            level,
        });
        if placement.blocks {
            entity.insert(crowd::InTheWay::object(level));
        }
        // An item's under a liquid's surface in a liquid cell, as a drop's (#3065).
        if placement.sinks {
            entity.insert(models::Sinks);
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
        mind.spell = placement.spell;
        mind.fun = placement.clips.fun_slots;
        mind.woman = placement.woman;
        if placement.trades != 0 {
            entity.insert(Trades(placement.trades));
        }
        if let Some(hire) = placement.hire {
            entity.insert(crate::followers::Hired(hire));
        }
        if placement.rises {
            entity.insert(RisesAgain);
        }
        if let Some(called) = placement.called {
            entity.insert(Called(called));
        }
        // `CreateNPC` wakes its creature of the flags' bit 0 as it makes it, its sight running
        // where it sees the hero (`0x483A03`, #3149).
        if placement.made.1 && word & 0x1 != 0 {
            entity.insert(crate::ai::Woken);
        }
        if placement.emerges {
            // In its glow as it emerges (`0x483311`, the event `0x109` of the state 13, #3074).
            entity.insert((
                ScriptClip::emerging(),
                crate::skins::Asked(
                    crate::skins::Overlay::Emerging,
                    crate::skins::EMERGING_SECONDS,
                ),
            ));
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
        mind.flies = record_flags & opensacred_rules::ways::FLIES != 0;
        let skills = fight::skills(&placement.base, grade, placement.plain);
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
        // A black rider's weapon on bronze's scale whatever the difficulty (`0x54B130`, #2338).
        let held_difficulty = if placement.rides {
            opensacred_rules::drops::Difficulty::Bronze
        } else {
            rules.difficulty()
        };
        let arms = Arms {
            base,
            plain: placement.plain,
            model: placement.model.clone(),
            holding: placement
                .holding
                .iter()
                .map(|&held| {
                    let values = templates.make(held.item, grade, (held_difficulty, 0), &mut dice);
                    (held, values)
                })
                .collect(),
            worn: placement
                .worn
                .iter()
                .map(|&(item, category)| {
                    let values = templates.make(item, grade, (rules.difficulty(), 0), &mut dice);
                    (item, category, values)
                })
                .collect(),
            mounted: placement.rides,
        };
        for (held, values) in &arms.holding {
            if let Some(values) = values {
                bevy::log::debug!("npcs: {creature} holds {}: {values:?}", held.item);
            }
        }
        // Its health, full, with its items' bonuses to its attributes (`Arms::most_health`,
        // #1365).
        let rank = placement
            .rank
            .health(placement.creature.0, placement.class, rules);
        let most = arms.most_health(grade, rank, rules);
        // A `SetHP`'s share of it (`0x48E280`, #1001), truncated.
        let current = placement
            .health_share
            .map_or(most, |share| most * share / 100);
        entity.insert(fight::Health {
            current,
            most,
            level: grade,
        });
        // One that a `SetHP` took to 0 before it was made dies as it comes, as one in the world
        // does ([`script_healths`]): quest 51's dying soldier, whose dialogue holds his body to be
        // spoken to (#2446, #2444).
        if current == 0 {
            entity.insert(Dies::unkilled());
        }
        let (values, hits) = arms.fight(grade, placement.rank, rules);
        // Its walk and run by its pace (`0x4F75F0`, #599).
        let pace = values.0.pace_factor();
        mind.speed = placement.speed[0] * pace;
        mind.run = placement.speed[1] * pace;
        mind.fly = minds::FLIES_AT * pace;
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
                names: placement.clips.clone().of_creature(creature),
                graph: None,
                player: None,
                hands_empty: placement.equipment.is_empty(),
                instrument: None,
                hits,
                pace,
            },
            crowd::InTheWay::of(record_flags, word, level),
            mind,
            arrows::Size(placement.size),
        ));
        // A horse, of its class (`+0x400`, 1 where none, #2299).
        if placement.class == HORSES_CLASS {
            entity.insert(crate::riding::Horse {
                class: opensacred_rules::horses::class(placement.horse_class),
            });
            // A stable's, of its trader (#2300).
            if let Some(trader) = &placement.stock {
                entity.insert(crate::stables::Stabled {
                    trader: trader.clone(),
                });
            }
        }
        // Its walk onto the floor its `NPC_Goto` names, of the building there, and for the
        // floor 0 onto its own level as the record runs, as it is made (`0x49E210`): the start's
        // novice, made outside the chapel, so stops inside its door, on the chapel's floor
        // where no way leads to her target on the ground (#2481).
        if let Some(((tx, ty), speed)) = placement.walk {
            let floor = placement.walk_floor;
            let onto = if floor > 0 {
                i32::try_from(tx)
                    .ok()
                    .zip(i32::try_from(ty).ok())
                    .and_then(|cell| heights.building_on_floor(cell, floor))
                    .and_then(|building| heights.floor_level(building, floor))
            } else {
                level
            };
            entity.insert(
                Walk::to(
                    iso::ground(tx as f32 + 0.5, ty as f32 + 0.5),
                    level,
                    speed * pace,
                )
                .onto(onto)
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
pub(crate) fn load_texture(assets: &AssetServer, texture: &str) -> Handle<Image> {
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
/// The type's flag that puts an FX object in the draw list 4 (`0x4280F0`, docs/re/drawing.md,
/// "The models"; #2941).
const LIST_4: u32 = 0x80_0000;

fn category(item: &Item) -> u8 {
    item.words_14[6].to_le_bytes()[2]
}

/// The model of an item that an NPC holds, attached to its hand (`equip`): the hand, its holder
/// being the NPC of its `models::OrderedWith`.
#[derive(Component, Clone, Copy, Debug)]
pub struct Wielded(Hand);

impl Wielded {
    /// Its hand's half of a weapon set, 0 the left and 1 the right, and its equipment's slot, 12
    /// and 13 (`0x556F50`).
    pub(crate) fn half(&self) -> usize {
        match self.0 {
            Hand::Left => 0,
            Hand::Right => 1,
        }
    }

    pub(crate) fn slot(&self) -> u8 {
        match self.0 {
            Hand::Left => 12,
            Hand::Right => 13,
        }
    }
}

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
    /// What it is made of, its hand and where it goes among them.
    piece: Piece,
    gltf: Handle<Gltf>,
    texture: Option<Handle<Image>>,
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

/// The equipment of the items `held`, their models and textures loading, to attach once its
/// holder's scene is ready ([`equip`]).
pub fn equipment(assets: &AssetServer, held: &[Held]) -> Equipment {
    Equipment(
        held.iter()
            .map(|held| Equip {
                piece: held.piece(),
                gltf: assets.load(held.model.clone()),
                texture: held.texture.as_ref().map(|t| load_texture(assets, t)),
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

    /// Whether every item is attached.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Takes out an item of `piece`, which its holder carries already: whether one was there.
    pub fn take(&mut self, piece: &Piece) -> bool {
        match self.0.iter().position(|item| item.piece == *piece) {
            Some(at) => {
                self.0.remove(at);
                true
            }
            None => false,
        }
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

/// The bone named `wanted` of `holder`'s own skeleton, the nearest its root, breadth first: never
/// one of the models of the items it holds or wears ([`Piece`]), whose sockets bear the same names,
/// as the original takes the creature's own bone (`0x555300`, `0x401AE0`). The Vampiress, whose
/// skeleton has no `Bone_weapon_01`, had a new weapon put on her shield's (#2512).
fn own_bone(
    holder: Entity,
    wanted: &str,
    (children, names, pieces): (&Query<&Children>, &Query<&Name>, &Query<(), With<Piece>>),
) -> Option<Entity> {
    let mut queue = std::collections::VecDeque::from([holder]);
    while let Some(entity) = queue.pop_front() {
        let Ok(kids) = children.get(entity) else {
            continue;
        };
        for &kid in kids.iter() {
            if pieces.contains(kid) {
                continue;
            }
            if names.get(kid).is_ok_and(|name| name.as_str() == wanted) {
                return Some(kid);
            }
            queue.push_back(kid);
        }
    }
    None
}

/// Attaches each NPC's equipment, once its scene is ready and the item's glTF and its scene are
/// loaded: the item's socket bone (see [`bones`]) goes onto the NPC's own bone that holds it
/// ([`own_bone`]). An item whose model lacks its socket is not drawn. The item's scene is spawned
/// only once it is loaded, so that it spawns at once: a spawn put off to a later frame panics in
/// Bevy's spawner when its holder is despawned meanwhile, as a hero's is by a new game started at
/// once (#488).
pub fn equip(
    mut commands: Commands,
    mut npcs: Query<(Entity, &mut Equipment)>,
    gltfs: Res<Assets<Gltf>>,
    worlds: Res<Assets<WorldAsset>>,
    nodes: Res<Assets<GltfNode>>,
    (children, names, pieces): (Query<&Children>, Query<&Name>, Query<(), With<Piece>>),
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
            if item.piece.mount == Mount::Worn {
                let Some(scene) = gltf.scenes.first().cloned() else {
                    return false;
                };
                if !worlds.contains(&scene) {
                    return true;
                }
                let mut piece = commands.spawn((
                    WorldAssetRoot(scene),
                    models::Source(item.gltf.clone()),
                    Transform::default(),
                    ChildOf(npc),
                    models::OrderedWith(npc),
                    WornBy(npc),
                    item.piece.clone(),
                ));
                if let Some(texture) = &item.texture {
                    piece.insert(Retexture(texture.clone()));
                }
                bevy::log::debug!("npcs: {npc} wears {}", piece.id());
                return false;
            }
            let (socket, own, holder) = bones(item.piece.hand, item.piece.mount);
            let Some(pose) = pose(gltf, &nodes, socket) else {
                return false;
            };
            let named = |wanted: &str| own_bone(npc, wanted, (&children, &names, &pieces));
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
                models::Source(item.gltf.clone()),
                Transform::from_matrix(pose.to_matrix().inverse()),
                ChildOf(holder),
                models::OrderedWith(npc),
                item.piece.clone(),
            ));
            if item.piece.mount == Mount::Back {
                weapon.insert(Shouldered);
            } else {
                weapon.insert(Wielded(item.piece.hand));
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
/// material copied once per texture with that texture as its base colour; a worn or held item's
/// model of an effect of the kind 9 its first primitive's material's copy that its effect draws
/// it by, which the type's texture never reaches (`0x406510`, `models::Bands`; #1309); the meshes
/// of a model of an effect of the kinds 1 to 4 or 8, a creature's, an object's or an item's, its
/// effect under them, with the kind 1's glow (`models::Effect`, #1834, #1848, #1850); and those
/// of a model of an effect of the kind 5 its pulse over them (`models::Pulse`, #1850). A model
/// finds its effect by its model's file, a worn or held item's, a creature's, else its glTF's: an
/// item on the ground, the interface's, a figure of the choice of the hero (#1875); the
/// interface's, which the original draws orthographic (`0x40F270`), has the kinds 1 to 4 alone.
/// A model takes its effect whatever the options' detail, which chooses as it draws whether the
/// effect is shown (`model_effects::shown`, #2943): `models` and `backpack::layer` keep a
/// mesh's plain material beside its effect's.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's query names the components it reads"
)]
pub fn retexture(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    models: Query<(
        Option<&Retexture>,
        Option<&Piece>,
        Option<&Npc>,
        Option<&models::Source>,
        Has<crate::backpack::Matte>,
    )>,
    children: Query<&Children>,
    mut meshes: Query<(
        &bevy::prelude::Mesh3d,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
    (mut materials, mut retextured): (ResMut<Assets<StandardMaterial>>, ResMut<Retextured>),
    (table, assets, mut effects): (
        Option<Res<crate::model_effects::Table>>,
        Res<AssetServer>,
        ResMut<crate::model_effects::Materials>,
    ),
) {
    let Ok((retexture, piece, npc, source, flat)) = models.get(ready.entity) else {
        return;
    };
    let path = source.and_then(|source| assets.get_path(source.0.id()));
    let model = piece
        .map(|piece| piece.model.as_str())
        .or(npc.map(|npc| npc.model.as_str()))
        .or_else(|| path.as_ref().and_then(|path| path.path().to_str()));
    let effects_of = model
        .zip(table.as_deref())
        .and_then(|(model, table)| table.of(model).map(|effect| (effect, table)));
    let effect = effects_of
        .map(|(effect, _)| effect)
        .filter(|&(kind, _)| !flat && kind == crate::model_effects::RUNNING_BANDS);
    // The kinds 1 to 4's effect, and the kind 1's glow, under every mesh, and the kind 8's
    // outside the interface (#1850).
    let under = effects_of
        .filter(|&((kind, _), _)| {
            crate::model_effects::under(kind) || (!flat && kind == crate::model_effects::HOLOGRAM)
        })
        .map(|((kind, texture), table)| effects.under(kind, (texture, table.glow()), &assets));
    // The kind 5's pulse over every mesh (#1850).
    let pulse = effects_of
        .filter(|&((kind, _), _)| !flat && kind == crate::model_effects::PULSE)
        .map(|((_, texture), _)| effects.pulse(texture, &assets));
    if retexture.is_none() && effect.is_none() && under.is_none() && pulse.is_none() {
        return;
    }
    for entity in children.iter_descendants(ready.entity) {
        let Ok((mesh, mut material)) = meshes.get_mut(entity) else {
            continue;
        };
        if let Some(under) = &under {
            commands.entity(entity).insert(under.clone());
        }
        if let Some(pulse) = &pulse {
            commands.entity(entity).insert(pulse.clone());
        }
        if let Some((_, texture)) = effect
            && crate::model_effects::first_primitive(&assets, mesh)
            && let Some(bands) =
                effects.running_bands(&material.0, texture, (&assets, &mut materials))
        {
            commands.entity(entity).insert(models::Bands(bands));
        }
        let Some(Retexture(texture)) = retexture else {
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
/// model's first, its walk and run clips, which play while the NPC walks or runs, and its
/// flight, stance, talk, fun, attack and dying clips, each where its model has it (`Clips`).
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
            let idle_clip = idle.clone();
            let idle = graph.add_clip(idle, 1.0, root);
            let mut add = |name| clip(name).map(|clip| graph.add_clip(clip.clone(), 1.0, root));
            let (walk, run, fly, stance, talk) = (
                add(&npc.names.walk),
                add(&npc.names.run),
                add(&npc.names.fly),
                add(&npc.names.stance),
                add(&npc.names.talk),
            );
            let names = npc.names.fun.clone();
            let fun = [0, 1, 2, 3].map(|i| add(&names[i]));
            let dying = npc
                .names
                .dying
                .iter()
                .filter_map(|(slot, name)| {
                    let clip = gltf.named_animations.get(name.as_str())?;
                    Some((*slot, graph.add_clip(clip.clone(), 1.0, root)))
                })
                .collect();
            let attacks = npc
                .names
                .attacks
                .iter()
                .filter_map(|name| {
                    let clip = gltf.named_animations.get(name.as_str())?.clone();
                    // A stand-in timed by the record 0 (`0x4EA1F0` reading the entry 0's, #3314).
                    let timed_by = if npc.names.stand_in {
                        LACKING.to_owned()
                    } else {
                        name.to_ascii_uppercase()
                    };
                    let hits = data.hits.get(&timed_by).copied().unwrap_or_default();
                    Some((graph.add_clip(clip.clone(), 1.0, root), clip, hits))
                })
                .collect();
            // The standing clips' second nodes, last, the others' nodes as they were.
            let idle_again = graph.add_clip(idle_clip, 1.0, root);
            let stance_again =
                clip(&npc.names.stance).map(|stance| graph.add_clip(stance.clone(), 1.0, root));
            npc.graph = Some(Clips {
                graph: graphs.add(graph),
                idle,
                walk,
                run,
                fly,
                stance,
                idle_again,
                stance_again,
                talk,
                fun,
                attacks,
                dying,
                scripted: HashMap::new(),
                layered: None,
            });
        }
        if let Some(clips) = npc.graph.clone() {
            let moving = walk.and_then(|walk| clips.moving(walk));
            let node = moving.unwrap_or(clips.idle);
            // The clips change through transitions, which blend them (`stance`); a walk's and a
            // run's play at its pace too (`0x413610`, #599).
            let rate = if moving.is_some() { npc.pace } else { 1.0 };
            let transitions = first_clip(&mut player, node, speed.0 * rate);
            commands
                .entity(entity)
                .insert((AnimationGraphHandle(clips.graph), transitions));
            npc.player = Some(entity);
        }
    }
}

/// Starts `player` on the clip `node` of its new graph, on and on at `speed`, and the transitions
/// that change its clips from there. A player whose graph [`rearm`] took keeps the old graph's
/// clips by their nodes, which the new graph gives to others, and they go first: a stale stance
/// looping on at what is now the dying clip's node made a corpse replay its death on and on, as
/// `AnimationPlayer::start` keeps an active clip's repeat (#1538).
fn first_clip(
    player: &mut AnimationPlayer,
    node: AnimationNodeIndex,
    speed: f32,
) -> AnimationTransitions {
    player.stop_all();
    let mut transitions = AnimationTransitions::new();
    transitions
        .play(player, node, Duration::ZERO)
        .repeat()
        .set_speed(speed);
    transitions
}

/// Walks each walking NPC toward its target as the hero walks (`hero::run`): round what blocks
/// the way and through the doors, at its walk speed times the game's, facing its way; at the
/// target it stands, its idle clip playing. A target beyond 25 cells along an axis it heads for
/// a leg at a time, its way searched to the cell brought nearer along the line (`paths::leg`,
/// `0x4FABE0`, #425). A target it cannot reach it walks straight toward, up to what blocks it,
/// and stands there; one whose first step is blocked stands a second in its idle before its walk
/// ends (#258).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
pub fn walk(
    mut commands: Commands,
    (time, tick): (Res<Time>, Res<WorldTick>),
    heights: Res<Heights>,
    // Who stands in the way, and who stays where it is made; the riders' seats and the horses
    // that charge (#2338).
    (crowd, still, seats, tramplers): (
        Res<crowd::Crowd>,
        Query<(), With<Still>>,
        Query<&crate::riding::Seat>,
        Query<(), With<crate::trample::Trampling>>,
    ),
    speed: Res<hero::GameSpeed>,
    mut walkers: Query<(
        Entity,
        &mut Transform,
        &mut Walk,
        &mut buildings::Layered,
        &mut crowd::InTheWay,
        (&Npc, Option<&crate::ai::Mind>),
        Has<crate::followers::Follower>,
    )>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for (entity, mut transform, mut walk, mut layered, mut way, (npc, mind), follows) in
        &mut walkers
    {
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
        // Held, its moving action takes no step until its timer fires, its walk ordered standing
        // (`0x4F479E`; docs/re/ai.md, "A held creature", #3622); a flyer's flight, a swoop's
        // among them, goes on (**hypothesis**: the swoop flies not by the moving action).
        if mind.is_some_and(|mind| mind.thinks.held()) && !walk.flies() {
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
        // The level it walks onto, as it will stand on it: on the ground of an entrance, the
        // building's inside, and on an exit, the ground, which the cell takes it onto as it
        // steps there (`0x5FCCB0`). Sent onto the ground of the chapel's door, the novice had
        // stepped onto it, inside, and gone back out through the exit before it, round at every
        // frame (#1438).
        let onto = walk
            .onto
            .map(|onto| heights.transition(paths::cell_of(target.x, target.z), onto));
        // A cell it may step into: walkable, no object and no other creature in it (`0x4F7E10`:
        // an NPC, of an id above 16, is stopped by any); aloft, any it flies onto, whoever stands
        // in it (`0x4EBCA0`, `0x4F7E10` for `+0x1F4` bit `0x200000`, #2250).
        let flies = walk.flies;
        let open = |cell: (i32, i32), at: Level| {
            if flies {
                heights.flown_onto(cell, at)
            } else {
                heights.walkable_on(cell, at)
            }
        };
        // A rider whose horse charges, the effect 24 on it, is stopped by no living creature, as
        // the hero's trampling horse is (`0x4F803E`, `0x4F82AE`, #2338).
        let charging = seats
            .get(entity)
            .ok()
            .and_then(crate::riding::Seat::ridden)
            .is_some_and(|horse| tramplers.contains(horse));
        let passes = flies || charging;
        let free = |cell, at| open(cell, at) && (passes || !crowd.stops(cell, at, entity, false));
        // A wide walker's radius by its type: its line along a lane of its square, its search
        // through cells whose corners within the radius pass too, but those it marked before
        // (`0x4FAA60`, `0x4F9410`, `0x4FB20A`–`0x4FB2CC`; #3359).
        let radius = mind.map_or(0, |mind| ways::radius(mind.creature));
        // Its leg and its course there, found as it sets out on the leg, anew off it (a stair's
        // landing) and at the leg's end short of its target, and again as it moves (#2175). The
        // leg is tested on the target's floor of the building there, else on the ground
        // (`0x4FABE0`'s test `0x4EBCA0` of `0x6354D0`'s floor), as the hero's (`hero::run`,
        // #1344, #2011).
        let floor = heights.floor_number(onto.unwrap_or(level));
        // Its place on its way, at its level and its floor number.
        let start = (level, heights.floor_number(level), false);
        // Searched again, for the point that `old` was searched for, `old` kept where the search
        // finds one alike (`paths::Planned::again`).
        let plan = |here: Vec3, old: Option<paths::Planned<Floored>>| {
            let leg = old.as_ref().map_or_else(
                || {
                    paths::leg(here, target, |cell| {
                        open(cell, heights.on_floor(cell, floor))
                    })
                },
                |old| old.sought,
            );
            let line = old.as_ref().is_none_or(|old| !old.searched());
            let end = paths::cell_of(leg.x, leg.z);
            // The goal's floor of the search over floor numbers, that of the level the leg's end
            // is at, as the hero's (`hero::run`, #2013).
            let goal = heights.floor_number(if leg == target {
                onto.unwrap_or(level)
            } else {
                heights.on_floor(end, floor)
            });
            let marks =
                (radius > 0).then(|| ways::wide_marks(cell, end, radius, |at| free(at, level)));
            let wide = |place: &((i32, i32), Floored)| {
                let (cell, (at, ..)) = *place;
                marks.as_ref().is_none_or(|marks| {
                    marks.contains(&cell)
                        || ways::wide_passes(cell, radius, |other| open(other, at))
                })
            };
            let neighbours = |place| {
                let mut next = heights.floored_neighbours(place, (start.1, goal), open);
                next.retain(|place| wide(place));
                paths::passable(next, end, |cell, (at, ..)| {
                    !passes && crowd.stops(cell, at, entity, false)
                })
            };
            let lane = |cell, at| {
                if radius == 0 {
                    free(cell, at)
                } else {
                    ways::square_passes(cell, radius, |other| free(other, at))
                }
            };
            let found = paths::course(
                here,
                start,
                leg,
                Some(goal),
                line,
                |cell, (at, ..)| lane(cell, at),
                neighbours,
                |(_, floor, _)| floor,
            )
            .map(|course| paths::Planned::new(leg, leg, course, here));
            match old {
                Some(old) => old.again(
                    found,
                    here,
                    start,
                    |cell, (at, ..)| lane(cell, at),
                    neighbours,
                    |(_, floor, _)| floor,
                ),
                None => found,
            }
        };
        let follow = |planned: &mut Option<paths::Planned<Floored>>| {
            planned.as_mut().and_then(|planned| {
                paths::follow(
                    &mut planned.course,
                    here,
                    start,
                    planned.leg,
                    |cell, (at, ..)| free(cell, at),
                    |(_, floor, _)| floor,
                )
            })
        };
        let at_leg = |planned: &Option<paths::Planned<Floored>>| {
            planned.as_ref().is_some_and(|planned| {
                (planned.leg - here).with_y(0.0).length() <= AT_LEG
                    && (planned.leg - target).length() > 0.0
            })
        };
        // Searched again at the world's ticks as it moves, every half second at the longest for
        // the hero's follower, every two and a half for the others (`0x4F4770`, #2175).
        let every = if follows {
            paths::PLAYERS_AGAIN
        } else {
            paths::OTHERS_AGAIN
        };
        if walk.course.is_none() || at_leg(&walk.course) {
            walk.course = plan(here, None);
            // No way to its target: one walking where it was sent heads for the last cell of the
            // line toward it that it may step into on its level, up to 14, which becomes its
            // target on that level (`0x4FB44A`, `+0x158`, `+0x15C`), and stands there. So the
            // start's novice, sent to the chapel's benches on the ground, stops inside its door,
            // where the chapel's mass ends her line (#2481).
            if walk.course.is_none() && walk.idle && !flies {
                let to = paths::cell_of(target.x, target.z);
                let end = ways::no_way_end(cell, to, |c| free(c, level));
                if end != to {
                    let point = paths::centre(end);
                    walk.target = Vec3::new(point.x, here.y, point.y);
                    walk.onto = Some(level);
                    bevy::log::debug!("npcs: {entity} finds no way to {to:?}, heads for {end:?}");
                    continue;
                }
            }
        } else if let (Some(seconds), Some(planned)) = (tick.seconds, walk.course.as_mut())
            && planned.due(paths::cell_of(here.x, here.z), seconds, every)
        {
            walk.course = plan(here, walk.course.take());
        }
        let mut step = follow(&mut walk.course);
        if step.is_none() && walk.course.is_some() {
            walk.course = plan(here, None);
            step = follow(&mut walk.course);
        }
        let (from, toward) = match step {
            Some(paths::Step::Through((next, ..))) => {
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
        // A step it may not take ends its walk, as quest 1's novice stopped at the hero put in
        // her way and stood there (seen in the original, #227's check 1).
        let blocked = into != cell && !free(into, walk.level) && open(cell, walk.level);
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
            // A flyer comes down in its flight's clip (`flight`, `stance`).
            if let Some(clips) = &npc.graph
                && !flies
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

/// A creature carried where another goes, a horse under its rider (`riding`, #2299): its walk
/// or its run playing (`Some(true)` the run), or standing.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Steered(pub Option<bool>);

/// How long a standing creature's clip blends into the next (`0x593320`).
const BLEND: Duration = Duration::from_millis(150);

/// The speed of a put back body's dying clip, which it runs through in a frame (#2457): a frame of
/// 1/60 s runs 16 s of it, more than any dying clip lasts.
const FALLEN_SPEED: f32 = 1000.0;
/// How near the hero's nearest hostile puts the standing creatures in their fight stance, in
/// world units (`0x546A7E`).
const STANCE_WITHIN: f32 = 550.0;

/// Puts each walking NPC in its walk or run clip, and each standing NPC whose model has a fight
/// stance into it while the hero's nearest hostile is within 550 units, wherever the NPC stands,
/// and back into its idle otherwise, blending over 0.15 s (`0x5467A0`, which a standing creature
/// asks at every update, `0x544890`; docs/re/motions.md, "The fight stance"). A standing one
/// that talks ([`Talks`]) or plays its instrument ([`Plays`]) plays that clip first, a flyer
/// aloft its flight's, and a horse under its rider its walk or its run as [`Steered`] tells.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
pub fn stance(
    mut commands: Commands,
    nearest: Res<crowd::Nearest>,
    npcs: Query<
        (
            Entity,
            &Npc,
            Option<&Walk>,
            Has<Talks>,
            Option<&Plays>,
            (Has<crate::flight::Aloft>, Option<&Steered>),
        ),
        (Without<Strikes>, Without<Dies>, Without<ScriptClip>),
    >,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    speed: Res<hero::GameSpeed>,
) {
    let fight = nearest.distance < STANCE_WITHIN;
    for (entity, npc, walk, talks, plays, (aloft, steered)) in &npcs {
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
        // A walking NPC walks; a standing one stands, in its stance where it fights; a flyer
        // aloft comes down in its flight's clip (`0x598F44`).
        // A horse carried under its rider walks or runs as the rider does (`riding`, #2299).
        let moving = match steered.map(|steered| steered.0) {
            Some(Some(true)) => clips.run.or(clips.walk),
            Some(Some(false)) => clips.walk,
            Some(None) => None,
            None => walk.and_then(|walk| clips.moving(walk)),
        };
        let flying = clips.fly.filter(|_| aloft);
        let wanted = match (moving, flying, fight, clips.stance) {
            (Some(moving), ..) => moving,
            (None, Some(fly), ..) => fly,
            (None, None, true, Some(stance)) => stance,
            _ => clips.idle,
        };
        if !transitions
            .get_main_animation()
            .is_some_and(|main| clips.same(main, wanted))
        {
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
/// its graph the first time, blended in over 0.25 s at its own rate, else its attack speed's
/// hundredth (`0x584FE0`; 1.0 for a creature of none), looped its count of times; it ends at the
/// clip's end, or 0.2 s before it for the slots 53 to 65 (`0x545204`), and the creature stands
/// again (`stance`). As the clip passes their shares, a cast's act is marked ([`ClipActs`]) and
/// an art's hits are sent ([`ArtHit`]). A slot its model lacks plays nothing, a cast of one
/// acting at once. Shaddar's clip of the slot 163 never ends: at each world's tick past its blend
/// in where the share of its span left is below 0.35 or above 0.85 it leaps back to its half,
/// with no blend, so that his body holds one stretch of it (`0x544C58`–`0x544DC3`,
/// `opensacred_rules::shaddar::held_clip_leaps`, #3449), until another clip takes its place.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's query carries its filters"
)]
fn script_clips(
    mut commands: Commands,
    (data, tick): (Res<Npcs>, Res<WorldTick>),
    mut npcs: Query<
        (
            Entity,
            &mut Npc,
            &mut ScriptClip,
            Option<&fight::Combat>,
            Option<&ai::Mind>,
        ),
        Without<Dies>,
    >,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    (gltfs, clips): (Res<Assets<Gltf>>, Res<Assets<AnimationClip>>),
    mut graphs: ResMut<Assets<AnimationGraph>>,
    (speed, factors, mut art_hits): (
        Res<hero::GameSpeed>,
        Res<crate::motions::Factors>,
        MessageWriter<ArtHit>,
    ),
) {
    for (entity, mut npc, mut clip, combat, mind) in &mut npcs {
        let Some(player_entity) = npc.player else {
            continue;
        };
        let Ok((mut player, mut transitions)) = players.get_mut(player_entity) else {
            continue;
        };
        if let Some((node, handle)) = clip.playing.clone() {
            let length = clips.get(&handle).map_or(0.0, AnimationClip::duration);
            if clip.slot == opensacred_rules::shaddar::HELD_SLOT
                && mind.is_some_and(|mind| mind.creature == SHADDAR)
            {
                if transitions.get_main_animation() != Some(node) {
                    bevy::log::debug!("npcs: {entity}'s held clip gives way to another");
                    commands.entity(entity).remove::<ScriptClip>();
                } else if tick.seconds.is_some()
                    && length > 0.0
                    && let Some(active) = player.animation_mut(node)
                    && active.elapsed() >= CLIP_IN.as_secs_f32()
                    && opensacred_rules::shaddar::held_clip_leaps(
                        (length - active.seek_time()).max(0.0) / length,
                    )
                {
                    if active.is_finished() {
                        active.replay();
                    }
                    active.seek_to(length / 2.0);
                    bevy::log::debug!("npcs: {entity}'s held clip leaps back to its half");
                }
                continue;
            }
            // A cast's act at its share of the clip, or as it ends.
            let acts = clip.acts_at.is_some_and(|share| {
                player.animation(node).is_none_or(|active| {
                    active.is_finished() || active.seek_time() >= length * share
                })
            });
            let ended = player.animation(node).is_none_or(|active| {
                active.is_finished()
                    || (DEFENDING.contains(&clip.slot)
                        && active.completions() + 1 >= clip.loops
                        && active.seek_time() >= length - DEFENDING_BACK)
            });
            if acts {
                commands.entity(entity).insert(ClipActs);
                clip.acts_at = None;
            }
            // An art's hit points as the clip passes them, the last at its end at the latest.
            let at = player
                .animation(node)
                .map_or(length, |active| active.seek_time());
            let finished = player
                .animation(node)
                .is_none_or(|active| active.is_finished());
            let shares = clip.hits.clone().unwrap_or_default();
            while let Some(&share) = shares.get(clip.passed) {
                if !finished && at < length * share {
                    break;
                }
                art_hits.write(ArtHit {
                    creature: entity,
                    n: clip.passed,
                });
                clip.passed += 1;
            }
            if ended {
                commands.entity(entity).remove::<ScriptClip>();
                if clip.slot == EMERGING {
                    commands.entity(entity).insert(Emerged);
                }
            }
            continue;
        }
        let name = data
            .slots
            .get(&npc.model)
            .and_then(|slots| slots.slot(clip.slot));
        // An art's clip's hit points by its name, its half where it has none and asks it.
        if clip.hits.is_some() {
            let floats = name
                .and_then(|name| data.points.get(&name.to_ascii_uppercase()))
                .cloned()
                .unwrap_or_default();
            let mut points = opensacred_rules::arts::hit_points(&floats);
            if points.is_empty() && clip.half {
                points.push(0.5);
            }
            clip.hits = Some(points);
        }
        if npc.graph.is_none() {
            continue;
        }
        let scripted = slot_clip(&mut npc, clip.slot, &data, (&gltfs, &mut graphs));
        let Some((node, handle)) = scripted else {
            bevy::log::debug!(
                "npcs: {entity} has no clip of the slot {} to play",
                clip.slot
            );
            // A cast without its clip acts at once (`0x51D150`, #1852).
            if clip.acts_at.is_some() {
                commands.entity(entity).insert(ClipActs);
            }
            commands.entity(entity).remove::<ScriptClip>();
            continue;
        };
        // Times its motion's own factor (`0x413610`, #3567).
        let rate = clip
            .rate
            .unwrap_or_else(|| combat.map_or(1.0, |combat| combat.0.speed / 100.0))
            * factors.of(&handle);
        let active = transitions.play(&mut player, node, CLIP_IN);
        active.replay();
        active
            .set_repeat(bevy::animation::RepeatAnimation::Count(clip.loops))
            .set_speed(speed.0 * rate);
        clip.playing = Some((node, handle));
    }
}

/// The node and the clip of the slot `slot` of an NPC's model in its graph, added to it the first
/// time it is asked (#1277); none where it has no graph or its model lacks the slot.
fn slot_clip(
    npc: &mut Npc,
    slot: u16,
    data: &Npcs,
    (gltfs, graphs): (&Assets<Gltf>, &mut Assets<AnimationGraph>),
) -> Option<(AnimationNodeIndex, Handle<AnimationClip>)> {
    let Npc {
        graph, model, gltf, ..
    } = npc;
    let graph = graph.as_mut()?;
    if let Some(scripted) = graph.scripted.get(&slot) {
        return Some(scripted.clone());
    }
    let name = data.slots.get(model.as_str())?.slot(slot)?;
    let handle = gltfs.get(&*gltf)?.named_animations.get(name)?.clone();
    let mut graph_asset = graphs.get_mut(&graph.graph)?;
    let root = graph_asset.root;
    let added = (graph_asset.add_clip(handle.clone(), 1.0, root), handle);
    graph.scripted.insert(slot, added.clone());
    Some(added)
}

/// The entity of an NPC's animation player, and the node of its hit clip laid over its clip in
/// play with its clip (`hit_clips`, #3637): its model's slot 175, added to its graph the first
/// time as a node apart from its scripts' clips, none where its model lacks the slot or it has no
/// player yet.
pub(crate) fn hit_layer(
    npc: &mut Npc,
    data: &Npcs,
    (gltfs, graphs): (&Assets<Gltf>, &mut Assets<AnimationGraph>),
) -> Option<(Entity, (AnimationNodeIndex, Handle<AnimationClip>))> {
    let player = npc.player?;
    let Npc {
        graph, model, gltf, ..
    } = npc;
    let graph = graph.as_mut()?;
    if let Some(layered) = graph.layered.clone() {
        return Some((player, layered));
    }
    let name = data
        .slots
        .get(model.as_str())?
        .slot(opensacred_rules::arts::HIT_CLIP)?;
    let handle = gltfs.get(&*gltf)?.named_animations.get(name)?.clone();
    let mut graph_asset = graphs.get_mut(&graph.graph)?;
    let root = graph_asset.root;
    let layered = (graph_asset.add_clip(handle.clone(), 1.0, root), handle);
    graph.layered = Some(layered.clone());
    Some((player, layered))
}

/// A creature whose clip of [`EMERGING`] has just ended ([`turn_emerged`]).
#[derive(Component)]
struct Emerged;

/// Turns each big demon whose clip of [`EMERGING`] has ended hostile (the standing action's
/// sub-state `0x66`, `0x544EAE`, #2461): its side 2 and its flags word's bit 0, by which the
/// hero's look wakes it, levels it where it is a boss of a marker level and takes the hero
/// for its foe; its kept placement with it, so that it comes back so with its sector or a save.
/// Quest 74's Sakkara demon, which `HornTot74` makes peaceful with [`EMERGES`], fights so some
/// 7 s after it is made. Another type stays as it was.
fn turn_emerged(
    mut commands: Commands,
    mut npcs: ResMut<Npcs>,
    mut emerged: Query<(Entity, &mut ai::Mind, Option<&Placed>), With<Emerged>>,
) {
    for (entity, mut mind, placed) in &mut emerged {
        commands.entity(entity).remove::<Emerged>();
        if mind.creature != BIG_DEMON {
            continue;
        }
        mind.side = 2;
        mind.flags |= 1;
        if let Some(Source::Kept(index)) = placed.map(|placed| &placed.source)
            && let Some(placement) = npcs.placements.get_mut(*index)
        {
            placement.side = 2;
            placement.creature.2 |= 1;
            placement.emerges = false;
        }
        bevy::log::info!("npcs: {entity}, a big demon, has emerged and turns hostile");
    }
}

/// Gives each big demon standing in the AI's state 0 the side 9 and its flags word's bit 0 at each
/// world's tick (`0x546038`; docs/re/fight.md, "Sides", #3074): one that its clip of
/// [`EMERGING`] turned to the side 2 stands on the side 9 at its next tick, which differs from
/// the side 2 toward the side 15 alone.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's query carries its filters"
)]
fn big_demons_stand(
    tick: Res<WorldTick>,
    mut demons: Query<
        &mut ai::Mind,
        (
            Without<Walk>,
            Without<Strikes>,
            Without<ScriptClip>,
            Without<Dies>,
        ),
    >,
) {
    if tick.seconds.is_none() {
        return;
    }
    for mut mind in &mut demons {
        if mind.creature == BIG_DEMON && mind.idle() && (mind.side != 9 || mind.flags & 1 == 0) {
            mind.side = 9;
            mind.flags |= 1;
        }
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

impl Door {
    /// Its type.
    pub fn kind(&self) -> u32 {
        self.kind
    }
}

/// An NPC rider that a door in its way gets off its horse (`0x53A170`, its mount its target;
/// `opensacred_rules::minds::rides_through`, #2293), which [`carry_riders`] puts down on foot.
#[derive(Component)]
pub struct GetsOff;

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
    /// among them (#1064, [`Self::change_kept`]); their `SetNPCState`s and `EquipNPC`s on the
    /// creatures they make ([`set_states`], #2752), and their `EquipNPC`s on those that earlier
    /// records made ([`Self::equip_made`], #2756). How many they placed, each placement made,
    /// what those `EquipNPC`s changed, and the `RegionChange`s for the creatures in the world
    /// ([`region_changes`]; [`Added`]).
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
        let mut into_pack = Vec::new();
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
            // Armed by the rules of the last `Enter` once it is placed (#3072).
            let mut placement = placement;
            if record.kind == CREATE_NPC && by_rules(ops) {
                placement.by_rules = self.last_enter;
                bevy::log::debug!(
                    "npcs: a {} of the scripts on {:?} is to be armed by {:?}'s rules",
                    placement.creature.0,
                    placement.cell,
                    self.last_enter
                );
            }
            let index = self.placements.len();
            if record.kind == CREATE_OBJ
                && let (Some(name), Some(trigger)) = (name_of(ops), placement.trigger.clone())
            {
                self.named.insert(name, trigger);
            }
            self.by_sector.entry(at).or_default().push(index);
            if placement.to_pack && placement.lies {
                into_pack.push(index);
            }
            self.placements.push(placement);
            self.played.insert(index);
            made.insert(std::ptr::from_ref(record), index);
            if self.spawned.contains(&at) {
                self.pending.push(index);
            }
        }
        let equips = set_states(&records, &made, &mut self.placements);
        self.makers
            .equip(equips, &mut self.placements, &mut self.slots)?;
        let equipped = self.equip_made(&records, &made)?;
        let born = records
            .iter()
            .enumerate()
            .filter_map(|(position, &record)| {
                made.get(&std::ptr::from_ref(record))
                    .map(|&index| (position, index))
            })
            .collect();
        Ok(Added {
            equipped,
            placed: made.len(),
            changes,
            born,
            into_pack,
        })
    }

    /// The item that the `CreateObj` of the placement `index` gives straight into the hero's
    /// backpack ([`TO_PACK`], #3543): as it would lie ([`laid_item`], by `hero` and `made`), with
    /// its script name, and its `Use:` function; none for no such placement.
    fn pack_item(
        &mut self,
        index: usize,
        (hero, made): (u32, Option<Laying>),
    ) -> Option<(opensacred_rules::drops::Dropped, Option<String>)> {
        let placement = self.placements.get(index)?.clone();
        let side = u32::try_from(SIDE).ok()?;
        let at = (placement.cell.0 / side, placement.cell.1 / side);
        let laid = laid_item(
            (&mut self.laid, self.values.get(&at), &self.fight),
            (index, &placement),
            (hero, made),
        );
        let dropped = opensacred_rules::drops::Dropped {
            script: placement.key.as_ref().map(|key| key.to_lowercase()),
            ..laid
        };
        Some((dropped, placement.uses))
    }

    /// What the play changed of the scripts' placements and triggers, as a save keeps it.
    pub fn saved(&self) -> Saved {
        Saved {
            records: self.records.clone(),
            opened: self.opened.clone(),
            marked: self.marked.clone(),
            locked: self.locked.clone(),
            taken: self.taken.clone(),
            emptied: self.emptied.clone(),
            laid: self.laid.clone(),
        }
    }

    /// Puts back what the play of a loaded save changed, on the placements of its start made
    /// again of its seed (#1958): its records of the world placed again batch by batch, as
    /// [`script_records`] placed them on the placements (their creatures and objects, places,
    /// objects' triggers, icons, states, groups and moves), none in the world yet, their
    /// creatures placed as their sectors come in (`area` the area of a sector); then the triggers
    /// open, marked and locked, the quests' items taken and the containers emptied as they stood,
    /// and the levels that the lying items were made at (#1125).
    pub fn resume(&mut self, saved: Saved, area: impl Fn((u32, u32)) -> Option<u8>) {
        for records in &saved.records {
            let born = match self.add(records, &area) {
                Ok(added) => added.born,
                Err(e) => {
                    bevy::log::warn!("npcs: the save's records of the world: {e:#}");
                    Vec::new()
                }
            };
            reicon(&self.dialogues, &mut self.icons, set_icons(records));
            self.restate(records);
            self.regroup(records, &born);
            self.moves(records, 0.0);
        }
        bevy::log::info!(
            "npcs: the save's {} batches of records of the world placed again",
            saved.records.len()
        );
        self.records = saved.records;
        self.opened = saved.opened;
        self.marked = saved.marked;
        self.locked = saved.locked;
        self.taken = saved.taken;
        self.emptied = saved.emptied;
        self.laid = saved.laid;
    }

    /// Puts back what the sectors of a loaded save kept of their creatures (#1959), on the
    /// placements of its start made again ([`Self::resume`]): the kept placements as the play left
    /// them, and where each creature stood or that it was killed, none in the world yet, so that
    /// each comes back as its sector comes in; the followers' placements stay out, which the save
    /// puts back beside the hero ([`follow_again`]).
    pub fn remember(&mut self, creatures: Creatures) {
        let side = u32::try_from(SIDE).unwrap_or(64);
        for (index, placement) in creatures.placements {
            let Some(kept) = self.placements.get_mut(index) else {
                continue;
            };
            let from = (kept.cell.0 / side, kept.cell.1 / side);
            let to = (placement.cell.0 / side, placement.cell.1 / side);
            *kept = placement;
            // One that a teleport moved away from the world comes in with the sector it was
            // put in (#2448).
            if from != to {
                self.file_under(index, to);
            }
        }
        let killed = creatures.kept.went.values().filter(|went| went.is_none());
        bevy::log::info!(
            "npcs: the save's {} kept creatures, {} killed, and the fills' of {} sectors",
            creatures.kept.went.len(),
            killed.count(),
            creatures.kept.filled.len()
        );
        self.kept = creatures.kept;
        self.queues = creatures.queues.into_iter().collect();
        self.entered = creatures.entered.into_iter().collect();
        // Each that lives away comes back with the sector of where it stood or is to be made,
        // which a save's play may have left it filed apart from (#2448, #2453), and each body
        // with the sector where it lies: a follower's with none (#2457).
        let moved: Vec<(usize, (u32, u32))> = self
            .placements
            .iter()
            .enumerate()
            .filter_map(|(index, placement)| {
                let (x, y) = self
                    .lives_away(index, placement)
                    .or_else(|| self.kept.lies(index))?;
                let sector = (x / side, y / side);
                let filed = self
                    .by_sector
                    .get(&sector)
                    .is_some_and(|list| list.contains(&index));
                (!filed).then_some((index, sector))
            })
            .collect();
        for (index, sector) in moved {
            self.file_under(index, sector);
        }
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
                Some(Some(stood)) if !stood.lies() => stood.cell,
                Some(_) => continue,
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
    /// once); a `SetCinemaMode` stops the hero first (#1280). With them, in the same order, what
    /// the other records do to the creatures they name ([`Act`]): an `Attack`'s teleport or cast
    /// ([`phased`], [`cast_by`]), a `PlayAnim` and the other records of [`played`], and under the
    /// mode a `CreateObj` made at a creature's turn ([`maker`]).
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
            if record.kind == ATTACK
                && let Some((name, whither)) = phased(ops, &self.places)
            {
                moves.push(Moved {
                    name,
                    act: Act::Phase(whither),
                    queued,
                });
                continue;
            }
            if record.kind == ATTACK
                && let Some((name, spell, at)) = cast_by(ops, &self.places)
            {
                moves.push(Moved {
                    name,
                    act: Act::Cast { spell, at },
                    queued,
                });
                continue;
            }
            if let Some(moved) = played(record.kind, ops, queued, now) {
                moves.push(moved);
                continue;
            }
            if queued && let Some(name) = maker(record) {
                moves.push(Moved {
                    name,
                    act: Act::Make(Box::new(record.clone())),
                    queued,
                });
                continue;
            }
            let Some(name) = mover(record.kind, ops, queued) else {
                continue;
            };
            let walks = record.kind == NPC_GOTO;
            // Its count of orders starting at 1 with the tag `0x66` (`0x49E210`), so that its
            // first order is a walk's (#1892).
            let strolls = walks && ops.iter().any(|o| o.tag == STROLLS);
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
                act: Act::Go {
                    whither,
                    walks,
                    strolls,
                },
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
                    state.restate_flags(placement);
                    state.restate_placement(placement);
                }
            }
            stated.push((name, state));
        }
        stated
    }

    /// What the records' `EquipNPC`s do to the creatures of their names that earlier records made
    /// (`0x481D80`, #2756), not those of the same records before them ([`set_states`]): the items
    /// each leaves on each placement of the name ([`Makers::equipped`]), given to it where it
    /// persists, the start's and the `Init`s' (an `Enter`'s is made again as its record runs),
    /// and returned by the placements' indices for the creatures in the world.
    fn equip_made(
        &mut self,
        records: &[&Record],
        made: &HashMap<*const Record, usize>,
    ) -> Result<Vec<Equipped>> {
        let batch: HashSet<usize> = made.values().copied().collect();
        let mut named: HashSet<String> = HashSet::new();
        let mut after: std::collections::BTreeMap<usize, (Vec<u32>, Vec<u32>)> = Default::default();
        for record in records {
            let Body::Operands(ops) = &record.body else {
                continue;
            };
            let Some(name) = name_of(ops) else {
                continue;
            };
            if record.kind == CREATE_NPC {
                named.insert(name);
                continue;
            }
            if record.kind != EQUIP_NPC || named.contains(&name) {
                continue;
            }
            let items: Vec<u32> = ops
                .iter()
                .filter_map(|o| match (o.tag, &o.value) {
                    (0x02, Value::I32(v)) => u32::try_from(*v).ok(),
                    _ => None,
                })
                .collect();
            for (index, placement) in self.placements.iter().enumerate() {
                if placement.object
                    || batch.contains(&index)
                    || placement.name.as_ref() != Some(&name)
                {
                    continue;
                }
                let (_, held) = after
                    .entry(index)
                    .or_insert_with(|| (placement.items.clone(), placement.items.clone()));
                *held = self.makers.equipped(held, &items);
            }
        }
        for (&index, (_, items)) in &after {
            let placement = &self.placements[index];
            if placement.made.0 {
                continue;
            }
            if let Some(equipped) =
                self.makers
                    .remade(placement, placement.creature.0, items, &mut self.slots)?
            {
                self.placements[index] = equipped;
            }
        }
        Ok(after
            .into_iter()
            .map(|(index, (before, held))| (index, before, held))
            .collect())
    }

    /// What the play's `SetGroupState`s and `DelNPC`s do (#1021): each given to the placements
    /// of its group or name here, but those that the records after it made (`born`, [`Added`]),
    /// a removal keeping them from being placed again, an object's of its name too (#2758), and
    /// returned, in order, for the creatures and the objects in the world ([`script_records`]).
    /// Quest 45's `Init45` removes Prince Valor and its
    /// `QIS_OnEnter45` makes him again at once, who stands so (#2448).
    fn regroup(&mut self, records: &[Record], born: &[(usize, usize)]) -> Vec<Regrouped> {
        let mut regrouped = Vec::new();
        for (position, record) in records.iter().enumerate() {
            let Body::Operands(ops) = &record.body else {
                continue;
            };
            let later: HashSet<usize> = born
                .iter()
                .filter(|&&(at, _)| at > position)
                .map(|&(_, index)| index)
                .collect();
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
                if !member || later.contains(&index) {
                    continue;
                }
                // An object of the name is destroyed for good (`0x497F80`), its sector's later
                // comings making it no more.
                if placement.object {
                    if matches!(change, Regrouped::Removed(_)) {
                        self.removed.insert(index);
                    }
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

    /// Where the placement `index` lives away from the world, wherever it stands: a placement
    /// not in the world now, not an `Enter`'s, made by the start or the play and never killed,
    /// at its own cell, or gone with its sector alive, at the cell where it stood. A sector's
    /// `Init` makes its creatures only as it comes near; the world's creatures are the caller's.
    fn lives_away(&self, index: usize, placement: &Placement) -> Option<(u32, u32)> {
        if placement.object || placement.made.0 {
            return None;
        }
        let made = placement.started || self.played.contains(&index);
        self.kept.away(index, placement.cell, made)
    }

    /// The scripts' groups that hold a creature living away from the world
    /// ([`Self::lives_away`]), as `GroupIsDead` asks of each member (`0x48C860`).
    pub fn living_groups(&self) -> HashSet<u32> {
        self.placements
            .iter()
            .enumerate()
            .filter_map(|(index, placement)| {
                let group = placement.group?;
                self.lives_away(index, placement).map(|_| group)
            })
            .collect()
    }

    /// The cell where the creature of the script name `name`, case aside, lives away from the
    /// world ([`Self::lives_away`]) or its body lies away from it (#2457), the first placement of
    /// that name that does, which a compass's marker points at as the original's finds the object
    /// of its name wherever it stands (`0x4A5980`, #2406).
    pub fn away(&self, name: &str) -> Option<(u32, u32)> {
        self.placements
            .iter()
            .enumerate()
            .filter(|(_, placement)| {
                placement
                    .name
                    .as_deref()
                    .is_some_and(|own| own.eq_ignore_ascii_case(name))
            })
            .find_map(|(index, placement)| {
                self.lives_away(index, placement)
                    .or_else(|| self.kept.lies(index))
            })
    }

    /// The cell of the script's object of the name `name`, case aside, that a `CreateObj` laid
    /// and that stands away from the world, its sector not in it: the first such placement that
    /// no taking and no `DelNPC` took away, which a compass's marker points at as the original's
    /// finds the object of its name wherever it stands (`0x4A5980`, #3105).
    pub fn object_away(&self, name: &str) -> Option<(u32, u32)> {
        let side = u32::try_from(SIDE).ok()?;
        self.placements
            .iter()
            .enumerate()
            .find(|(index, placement)| {
                placement.object
                    && !self.taken.contains(index)
                    && !self.removed.contains(index)
                    && !self
                        .spawned
                        .contains(&(placement.cell.0 / side, placement.cell.1 / side))
                    && placement
                        .name
                        .as_deref()
                        .is_some_and(|own| own.eq_ignore_ascii_case(name))
            })
            .map(|(_, placement)| placement.cell)
    }

    /// Clears the sight's bit of the kept placement `index` (`minds::SIGHT`, `0x59F78F`, #2475):
    /// its creature saw the hero, and comes back so with its sector or a save.
    pub(crate) fn sighted(&mut self, index: usize) {
        if let Some(placement) = self.placements.get_mut(index) {
            placement.creature.2 &= !minds::SIGHT;
        }
    }

    /// Puts the creatures of the script name `name`, case aside, that live away from the world
    /// ([`Self::lives_away`]) on the cell `(x, y)` at `layer`, as a script's `Teleport` moves a
    /// creature wherever its sector stands (`0x491D40`, `sdk_ENGINE_TP`; #2448): where it stood,
    /// or where it is to be made, undrawn about its place, the sector of that cell placing it as
    /// it comes in, at once where that sector is in the world already. Whether any was.
    pub fn put_away(&mut self, name: &str, (x, y): (i32, i32), layer: i32) -> bool {
        let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
            return false;
        };
        let side = u32::try_from(SIDE).unwrap_or(64);
        let to = (x / side, y / side);
        let indices: Vec<usize> = self
            .placements
            .iter()
            .enumerate()
            .filter(|&(index, placement)| {
                placement
                    .name
                    .as_deref()
                    .is_some_and(|own| own.eq_ignore_ascii_case(name))
                    && self.lives_away(index, placement).is_some()
            })
            .map(|(index, _)| index)
            .collect();
        for &index in &indices {
            let placement = &mut self.placements[index];
            placement.cell = (x, y);
            placement.layer = layer;
            placement.spread = 0;
            if let Some(Some(stood)) = self.kept.went.get_mut(&index) {
                stood.cell = (x, y);
                stood.layer = layer;
            }
            self.file_under(index, to);
            let now = self.spawned.contains(&to);
            if now {
                self.pending.push(index);
            }
            bevy::log::debug!(
                "npcs: {name}, placement {index}, put away on {x},{y}{}",
                if now { ", its sector in the world" } else { "" }
            );
        }
        !indices.is_empty()
    }

    /// Files the placement `index` under the sector `to` alone, which places it as it comes in:
    /// out of the list of any other, where a save's places of the play may have left it, its cell
    /// elsewhere (#2448).
    fn file_under(&mut self, index: usize, to: (u32, u32)) {
        for (sector, list) in &mut self.by_sector {
            if *sector != to {
                list.retain(|&other| other != index);
            }
        }
        let list = self.by_sector.entry(to).or_default();
        if !list.contains(&index) {
            list.push(index);
        }
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

/// Opens or closes each door that `ai`, the hero or the scripts swing ([`Swing`]), playing its
/// clip once, and its sound of opening or closing before it, at the door, at a rate's factor of
/// 0.45 and a tenth's roll (`0x44BCD0`, `0x695800`; #1166).
fn swing(
    mut commands: Commands,
    mut doors: Query<(Entity, &mut Door, &Swing)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    mut npcs: ResMut<Npcs>,
    mut heights: ResMut<Heights>,
    speed: Res<hero::GameSpeed>,
    (mut sounds, settings): (
        MessageWriter<sfx::Play>,
        Option<Res<crate::settings::Settings>>,
    ),
) {
    // No door's sound at the lowest sound quality (`0x695865`).
    let quiet = crate::settings::low_sound(settings.as_deref());
    let mut sound = |entity, door: &Door, open: bool| {
        if door.open == open || quiet {
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

/// Opens the trigger `name` of a container as it opens (`0x44BCD0`, #1380): its cells, if it has
/// any (a dwarves' chest's `chestTrigger`), freed.
pub(crate) fn open_container_trigger(npcs: &mut Npcs, heights: &mut Heights, name: &str) {
    open_trigger(npcs, heights, name, true);
}

/// Closes the trigger `name` of a town stash as its window hides (`0x416280`, #1411).
pub(crate) fn close_container_trigger(npcs: &mut Npcs, heights: &mut Heights, name: &str) {
    open_trigger(npcs, heights, name, false);
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
/// placement changes with it. The followers, hired already or to be hired (#2839), and the
/// creatures of no placement (the console's, the summons) are not changed.
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
        (
            Without<Dies>,
            Without<crate::followers::Hired>,
            Without<crate::followers::Follower>,
        ),
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
        streamer.adopt(&mut commands, sector, made);
    }
}

/// Carries out the scripts' `SetHP`s (`0x48E280`, #1001): the creatures of the name, the hero for
/// none, take the share of their most health, truncated; a creature of none dies, as by no
/// player's hand (it drops nothing).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
pub(crate) fn script_healths(
    mut commands: Commands,
    healths: Option<ResMut<crate::scripts::Healths>>,
    mut creatures: Query<(
        Entity,
        &ScriptName,
        &mut fight::Health,
        Option<&Stashed>,
        Has<Dies>,
    )>,
    mut heroes: Query<&mut fight::Health, (With<hero::Body>, Without<ScriptName>)>,
) {
    let Some(mut healths) = healths else {
        return;
    };
    // Those lifted by an earlier `SetHP` of the same run, which a later one finds standing: the
    // Underworld's `the_end_of_uw` lifts Vilya at 1 % and lays her down again at 0.
    let mut lifted = HashSet::new();
    for set in std::mem::take(&mut healths.0) {
        let Some(name) = &set.name else {
            for mut health in &mut heroes {
                health.current = health.most * set.share / 100;
            }
            bevy::log::info!("npcs: the scripts set the hero's health to {} %", set.share);
            continue;
        };
        for (entity, named, mut health, stashed, dead) in &mut creatures {
            if &named.0 != name {
                continue;
            }
            let dead = dead && !lifted.contains(&entity);
            // A dead one taken above 0 rises where it lies, from any function (`0x48E280`,
            // its stop `0x54E6D0`, which a health below 2 skips; #2477): the Underworld's
            // `refresh_a` brings Anducar back.
            if dead {
                if set.share == 0 {
                    continue;
                }
                health.current = health.most * set.share / 100;
                if let Some(stashed) = stashed
                    && health.current >= 2
                {
                    lift(&mut commands, entity, stashed);
                    lifted.insert(entity);
                    bevy::log::info!(
                        "npcs: the scripts raise {name} at {}/{}",
                        health.current,
                        health.most
                    );
                }
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
                lifted.remove(&entity);
            }
        }
    }
}

/// A creature whose queue is gone waits no more: it hears its foes again (#3253).
fn hear_again(gone: On<Remove<ScriptQueue>>, mut minds: Query<&mut ai::Mind>) {
    if let Ok(mut mind) = minds.get_mut(gone.entity)
        && mind.thinks.deaf()
    {
        mind.thinks.waits(false);
    }
}

/// Whether a creature of the type `creature` takes a `PlayAnim` outside the anim mode: its
/// receive takes the event `0x108` for the type 700 alone, Shaddar (`0x52E8AB`, #3252).
fn clip_outside_the_mode(creature: u32) -> bool {
    creature == SHADDAR
}

/// Lifts the dead creature `entity` out of its death (`0x59E151`'s `PlayAnim` over the dying
/// action, `0x48E280`'s stop `0x54E6D0`; #2477): its mind, out of its fight, and its place in the
/// way back from what it kept aside as it died ([`Stashed`]), its dying and its body's mark gone.
pub(crate) fn lift(commands: &mut Commands, entity: Entity, stashed: &Stashed) {
    let mut mind = stashed.mind.clone();
    mind.rises();
    commands
        .entity(entity)
        .remove::<(Dies, Corpse, Stashed)>()
        .insert((mind, stashed.way));
}

/// The steps that the play's records give a creature under the scripts' anim mode (#1220;
/// docs/re/netscript.md, "The anim mode and the creatures' queues"): its queue (`+0x588`), whose
/// first step it takes at each tick of the world ([`take_steps`], `0x59A840`), its own idle
/// behaviours left meanwhile (`ai`).
#[derive(Component, Default, Debug, Clone)]
pub struct ScriptQueue(std::collections::VecDeque<Queued>);

impl ScriptQueue {
    /// Whether no step waits.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Its steps as a save keeps them ([`SavedQueue`]), the real clock at `now` seconds.
    pub fn saved(&self, now: f64) -> SavedQueue {
        SavedQueue(self.0.iter().map(|step| step.saved(now)).collect())
    }
}

/// A creature's [`ScriptQueue`] as a save keeps it (#2890; the original saves each creature's
/// queue with it, `0x54B3D0`, docs/re/netscript.md): its steps, one begun taken again from its
/// start (a teleport's or a cast's, a cast at a creature made at the place it was ordered at), a
/// `Partikel`'s caster, of the run that saved, left out, and a `Wait`'s deadline of the real
/// clock kept as the seconds left.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedQueue(Vec<Queued>);

impl SavedQueue {
    /// Whether it holds no step.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The queue that a load gives back, the real clock at `now` seconds.
    fn loaded(self, now: f64) -> ScriptQueue {
        ScriptQueue(
            self.0
                .into_iter()
                .map(|step| match step {
                    Queued::Wait {
                        other,
                        until: Until::Time(left),
                    } => Queued::Wait {
                        other,
                        until: Until::Time(now + left),
                    },
                    step => step,
                })
                .collect(),
        )
    }

    /// Gives `entity` this queue as the world's real clock stands then, where it holds a step.
    pub fn give(self, commands: &mut Commands, entity: Entity) {
        if self.is_empty() {
            return;
        }
        commands.queue(move |world: &mut bevy::prelude::World| self.give_now(world, entity));
    }

    /// [`Self::give`] in the world at once.
    pub fn give_now(self, world: &mut bevy::prelude::World, entity: Entity) {
        if self.is_empty() {
            return;
        }
        let now = world
            .get_resource::<Time<bevy::time::Real>>()
            .map_or(0.0, Time::<bevy::time::Real>::elapsed_secs_f64);
        let steps = self.0.len();
        if let Ok(mut creature) = world.get_entity_mut(entity) {
            creature.insert(self.loaded(now));
            bevy::log::info!("npcs: {entity} takes its saved queue of {steps} steps again");
        }
    }
}

/// A step of a creature's [`ScriptQueue`].
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
enum Queued {
    /// A walk to `to` (`NPC_Goto`, the kind 1) onto the level `onto`, the creature's own as it
    /// sets out where the record names no floor, where the original takes the creature's layer as
    /// the record runs (`0x49E210`), so that its walk after a teleport down never counts as
    /// arrived and is dropped standing at its target (docs/decisions/netscript.md, "Queue's walk
    /// onto the level it sets out on"); how many times it was ordered, from 1 for a record of the
    /// tag `0x66` (#1892), and where the creature stood as the count began. Each order is a run
    /// where its count is 0, else a walk (`+0x1F8`, `0x59A8D9`).
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
    /// Another record's turn, of its kind, done at once (#1277): an `Attack` that neither
    /// teleports nor casts ([`QUEUED_OTHERS`], #1830).
    Other(u16),
    /// A `PlaySound`'s sound of the id, played once in the middle at its sample's rate and the
    /// volume 1 (the step `0x59ED41`, `playSFX` `0x693510`), done at once (#1826).
    Sound(u32),
    /// A `Partikel`'s effect at the creature of its first name, `caster` as the record ran (the
    /// entry's `+0x14`), done at once (the step `0x59CE78`, #1890).
    Partikel {
        at: String,
        #[serde(skip)]
        caster: Option<Entity>,
        numbers: Vec<i32>,
    },
    /// A `CreateObj`'s record, made at its turn (the step `0x59A9EF`), done at once (#1826).
    Make(Box<Record>),
    /// An `ActObj`'s door of the object of the name opened or closed (the step `0x59E9D5`), done
    /// at once (#1827).
    Obj { object: String, open: bool },
    /// A `Morph` of the creature of the name into the type `creature`, its level and health kept
    /// (the step `0x59F266`), done at once (#1829).
    Morph { name: String, creature: u32 },
    /// An `Attack`'s teleport (the step `0x59E3D6` sending `0x105`): its cast begun at its turn,
    /// waited for while it lasts (`script_attacks::Phase`, #1831).
    Phase {
        phase: crate::script_attacks::Phase,
        begun: bool,
    },
    /// An `Attack`'s cast of a spell at its target (the step `0x59E3D6` sending `0x105`): ordered
    /// at its turn, waited for while it lasts (`creature_casts::ScriptCast`, #1871).
    Cast {
        cast: crate::creature_casts::ScriptCast,
        begun: bool,
    },
    /// A `CallFunktion`'s function, run at its turn and done the same tick (the step `0x59CE5B`,
    /// #1278).
    Call(String),
    /// A `Wait` (the kind 2): holds the queue while its condition `until` holds of the other
    /// creature, `None` the hero (#1279).
    Wait { other: Option<String>, until: Until },
}

impl Queued {
    /// The step as a save keeps it ([`SavedQueue`]), the real clock at `now` seconds.
    fn saved(&self, now: f64) -> Self {
        match self {
            Self::Phase { phase, .. } => Self::Phase {
                phase: phase.again(),
                begun: false,
            },
            Self::Cast { cast, .. } => Self::Cast {
                cast: *cast,
                begun: false,
            },
            Self::Partikel { at, numbers, .. } => Self::Partikel {
                at: at.clone(),
                caster: None,
                numbers: numbers.clone(),
            },
            Self::Wait {
                other,
                until: Until::Time(deadline),
            } => Self::Wait {
                other: other.clone(),
                until: Until::Time((deadline - now).max(0.0)),
            },
            step => step.clone(),
        }
    }
}

/// A `Wait`'s condition (the step `0x59CAA5`, the table `0x59F538`), which holds the queue while
/// it holds.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
/// A scripts' clip as a save keeps it (#3759): all but its node, its clip played again from its
/// start as a load's.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedScriptClip {
    slot: u16,
    loops: u32,
    acts_at: Option<f32>,
    rate: Option<f32>,
    parry: bool,
    hits: Option<Vec<f32>>,
    half: bool,
    passed: usize,
}

impl MapEntities for SavedScriptClip {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, _: &mut E) {}
}

impl crate::doings::Saves for ScriptClip {
    type Saved = SavedScriptClip;

    fn saved(&self, _: &crate::doings::Moment) -> Option<SavedScriptClip> {
        Some(SavedScriptClip {
            slot: self.slot,
            loops: self.loops,
            acts_at: self.acts_at,
            rate: self.rate,
            parry: self.parry,
            hits: self.hits.clone(),
            half: self.half,
            passed: self.passed,
        })
    }

    fn again(saved: SavedScriptClip, _: &crate::doings::Moment) -> Option<Self> {
        Some(Self {
            slot: saved.slot,
            loops: saved.loops,
            playing: None,
            acts_at: saved.acts_at,
            rate: saved.rate,
            parry: saved.parry,
            hits: saved.hits,
            half: saved.half,
            passed: saved.passed,
        })
    }
}

#[derive(Component, Debug)]
pub struct ScriptClip {
    slot: u16,
    loops: u32,
    playing: Option<(AnimationNodeIndex, Handle<AnimationClip>)>,
    /// The share of a creature's cast's clip at which its spell acts (`0x521D60`; #1852), which
    /// [`ClipActs`] marks once it passes; none for another clip.
    acts_at: Option<f32>,
    /// The rate it plays at where not its attack speed's (a trampled creature's fall, 1.0).
    rate: Option<f32>,
    /// Whether it is a parry ([`ScriptClip::parry`]).
    parry: bool,
    /// An art's clip's hit points, shares of the clip, found as it starts to play, and whether
    /// it hits at its half where it has none ([`ScriptClip::art`]); and how many it has passed.
    hits: Option<Vec<f32>>,
    half: bool,
    passed: usize,
}

/// A hit point of a creature's art's clip that its clip passed, the `n`-th from 0
/// (`creature_arts`, #2564).
#[derive(bevy::ecs::message::Message, Clone, Copy, Debug)]
pub struct ArtHit {
    pub creature: Entity,
    pub n: usize,
}

impl ScriptClip {
    /// The clip of an idle creature's fidget, once, where it stands (`0x54EA30`, #1364).
    pub(crate) fn fidget(slot: u16) -> Self {
        Self::new(slot, 1)
    }

    /// The fall of a creature that a horse tramples, once at the rate 1.0 (`0x5941A0` at
    /// `0x5975C9`; `trample`, #2302).
    pub(crate) fn fall(slot: u16) -> Self {
        Self {
            rate: Some(1.0),
            ..Self::new(slot, 1)
        }
    }

    fn new(slot: u16, loops: u32) -> Self {
        Self {
            slot,
            loops,
            playing: None,
            acts_at: None,
            rate: None,
            parry: false,
            hits: None,
            half: false,
            passed: 0,
        }
    }

    /// A creature's weapon art's clip of the slot `slot`, once, an [`ArtHit`] at each of its hit
    /// points, or at its half where `half` and it has none (`creature_arts`, #2564).
    pub(crate) fn art(slot: u16, half: bool) -> Self {
        Self {
            hits: Some(Vec::new()),
            half,
            ..Self::new(slot, 1)
        }
    }

    /// A creature's boon art's clip of the slot `slot`, once at `rate` (none: its attack speed's),
    /// marked as it passes the share `share`, where the art gives the creature its effects
    /// (`creature_arts`, #2899).
    pub(crate) fn boon(slot: u16, rate: Option<f32>, share: f32) -> Self {
        Self {
            acts_at: Some(share),
            rate,
            ..Self::new(slot, 1)
        }
    }

    /// A creature's parry, its defending clip of the slot `slot` once in place of a strike, as
    /// a script's `PlayAnim` plays one (`0x4E4F50`–`0x4E4F85`, #2487).
    pub(crate) fn parry(slot: u16) -> Self {
        Self {
            parry: true,
            ..Self::new(slot, 1)
        }
    }

    /// Whether it is a parry, during which its creature neither decides nor strikes.
    pub(crate) fn parries(&self) -> bool {
        self.parry
    }

    /// Whether it is a defending clip, the slots 53 to 65, a parry's or a script's, by which its
    /// creature's defence doubles against a strike (`0x56B96A`–`0x56B9A5`, #2487).
    pub(crate) fn defends(&self) -> bool {
        DEFENDING.contains(&self.slot)
    }

    /// The slot it plays.
    pub(crate) fn slot(&self) -> u16 {
        self.slot
    }

    /// The rate it plays at where not its attack speed's.
    #[cfg(test)]
    pub(crate) fn rate(&self) -> Option<f32> {
        self.rate
    }

    /// The length of its clip in seconds, once it plays, at its own rate.
    pub(crate) fn length(&self, clips: &Assets<AnimationClip>) -> Option<f32> {
        let (_, clip) = self.playing.as_ref()?;
        clips.get(clip).map(AnimationClip::duration)
    }

    /// The clip that a `CreateNPC`'s [`EMERGES`] plays once, at the rate 1.0 (`0x544B6A`).
    fn emerging() -> Self {
        Self {
            rate: Some(1.0),
            ..Self::new(EMERGING, 1)
        }
    }

    /// A dryad's leap's clip of the slot `slot`, once at `rate`, its flight starting at the share
    /// `flies` ([`ClipActs`], `creature_arts::Leaps`, #2566).
    pub(crate) fn leap(slot: u16, rate: f32, flies: f32) -> Self {
        Self {
            acts_at: Some(flies),
            rate: Some(rate),
            ..Self::new(slot, 1)
        }
    }

    /// A creature's throw of a trap, its clip of the slot `slot` once at the rate 1.0, the ball
    /// thrown at the share `share` (`creature_casts`, #2731).
    pub(crate) fn thrown(slot: u16, share: f32) -> Self {
        Self {
            acts_at: Some(share),
            rate: Some(1.0),
            ..Self::new(slot, 1)
        }
    }

    /// A creature's cast's clip of the slot `slot`, once, its spell acting by its own timer: a
    /// script's teleport's (`script_attacks`, #2753).
    pub(crate) fn cast(slot: u16) -> Self {
        Self::new(slot, 1)
    }

    /// A creature's cast's clip of the slot `slot`, once, its spell acting at the share `share`
    /// (`creature_casts`, #1852).
    pub(crate) fn casting(slot: u16, share: f32) -> Self {
        Self {
            acts_at: Some(share),
            ..Self::new(slot, 1)
        }
    }
}

/// A creature whose cast's clip has passed the share at which its spell acts, or that has no
/// clip of its cast's slot and so acts at once (`0x51D150`; `creature_casts`, #1852).
#[derive(Component, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ClipActs;

impl MapEntities for ClipActs {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, _: &mut E) {}
}

impl crate::doings::Saves for ClipActs {
    type Saved = ClipActs;

    fn saved(&self, _: &crate::doings::Moment) -> Option<Self> {
        Some(*self)
    }

    fn again(saved: Self, _: &crate::doings::Moment) -> Option<Self> {
        Some(saved)
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
/// puts the creature there at once; the other steps as [`Queued`] tells each. An entry done with
/// more waiting leaves the creature's fight, unless it casts (`0x59F34A`, the AI's state 0 but
/// in the states 6 and 11; #1874). The queue goes once empty.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn take_steps(
    mut commands: Commands,
    tick: Res<WorldTick>,
    (heights, real): (Res<Heights>, Res<Time<bevy::time::Real>>),
    (mut due, mut sounds, mut made, mut states, mut particles): (
        Option<ResMut<crate::scripts::Due>>,
        MessageWriter<sfx::Play>,
        Option<ResMut<crate::scripts::WorldRecords>>,
        Option<ResMut<crate::scripts::ObjStates>>,
        Particles,
    ),
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
            Option<&mut Walk>,
            (
                Has<ScriptClip>,
                Has<Plays>,
                Has<crate::script_attacks::Phase>,
                (
                    Has<crate::creature_casts::ScriptCast>,
                    Has<crate::creature_casts::Casting>,
                ),
            ),
            (Option<&Stashed>, Has<Dies>),
        ),
        Without<hero::Body>,
    >,
) {
    if tick.seconds.is_none() {
        return;
    }
    for (
        entity,
        mut queue,
        mut transform,
        mind,
        way,
        layered,
        mut walk,
        (clip, plays, phasing, (ordered, casting)),
        (stashed, dead),
    ) in &mut creatures
    {
        let walking = walk.is_some();
        let here = transform.translation;
        let level = way.as_ref().and_then(|way| way.level);
        let done = match queue.0.front_mut() {
            None => true,
            // A dead one walks, casts and phases nowhere; the rest of its queue runs on (#2477).
            Some(Queued::Walk { .. } | Queued::Cast { .. } | Queued::Phase { .. }) if dead => true,
            Some(Queued::Walk {
                to,
                onto,
                orders,
                noted,
            }) => {
                if walking || mind.is_some_and(|mind| mind.fighting()) {
                    // Steered while it goes ([`Walk::steer`]).
                    if let Some(walk) = walk.as_deref_mut() {
                        walk.steer();
                    }
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
                        // At a run where its count is 0, else at a walk (`+0x1F8`, `0x59A8D9`).
                        let walk = if *orders == 0 {
                            *noted = here;
                            Walk::run(*to, level, mind.map_or(0.0, |mind| mind.run))
                        } else {
                            Walk::to(*to, level, mind.map_or(0.0, |mind| mind.speed))
                        };
                        *orders += 1;
                        commands.entity(entity).insert(walk.onto(onto).idle());
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
            // On a dead one it lifts it out of its death and plays from standing (#2477); one
            // that kept nothing aside to stand again takes no clip.
            Some(&mut Queued::Clip { slot }) if dead => {
                if let Some(stashed) = stashed {
                    lift(&mut commands, entity, stashed);
                    commands.entity(entity).insert(ScriptClip::new(slot, 1));
                }
                true
            }
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
            Some(&mut Queued::Sound(id)) => {
                sounds.write(queued_sound(id));
                true
            }
            Some(Queued::Partikel {
                at,
                caster,
                numbers,
            }) => {
                partikel_turn((at, *caster, numbers), &mut particles);
                true
            }
            // Made as the scripts' records of the world are, at once now that it is alone.
            Some(Queued::Make(record)) => {
                if let Some(made) = made.as_deref_mut() {
                    made.0.push((**record).clone());
                }
                true
            }
            Some(Queued::Obj { object, open }) => {
                if let Some(states) = states.as_deref_mut() {
                    states.0.push(act_on(object, *open));
                }
                true
            }
            // Ordered at its turn, and done once its cast ends (#1871).
            Some(Queued::Cast { cast, begun }) => {
                if *begun {
                    !ordered && !casting
                } else {
                    commands.entity(entity).insert(*cast);
                    *begun = true;
                    false
                }
            }
            // Begun at its turn, and done once the creature stands at its end.
            Some(Queued::Phase { phase, begun }) => {
                if *begun {
                    !phasing
                } else {
                    commands.entity(entity).insert(*phase);
                    *begun = true;
                    false
                }
            }
            // Turned as the scripts' `Morph`s are, its level and health kept ([`morphs`]).
            Some(Queued::Morph { name, creature }) => {
                if let Some(made) = made.as_deref_mut() {
                    made.0.push(Record {
                        kind: MORPH,
                        body: Body::Operands(vec![
                            opensacred_data::netscript::Operand {
                                tag: 0x01,
                                value: Value::Str(name.clone()),
                            },
                            opensacred_data::netscript::Operand {
                                tag: 0x02,
                                value: Value::I32(i32::try_from(*creature).unwrap_or(0)),
                            },
                            opensacred_data::netscript::Operand {
                                tag: QUEUED_MORPH,
                                value: Value::Marker,
                            },
                        ]),
                    });
                }
                true
            }
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
                let deaf = holds && until.stops();
                if deaf {
                    commands.entity(entity).remove::<Walk>();
                }
                // Held, it takes no foes' event, does not part and is stopped where it fights
                // (`+0x200` bit `0x10`, `0x54E6D0`, #3253); done, it hears them again.
                if mind.is_some_and(|mind| mind.thinks.deaf() != deaf || (deaf && mind.fighting()))
                {
                    commands.entity(entity).queue(
                        move |mut creature: bevy::ecs::world::EntityWorldMut| {
                            if let Some(mut mind) = creature.get_mut::<ai::Mind>() {
                                mind.thinks.waits(deaf);
                            }
                        },
                    );
                }
                !holds
            }
        };
        if done {
            queue.0.pop_front();
            if !queue.0.is_empty()
                && mind.is_some_and(|mind| mind.fighting())
                && !ordered
                && !casting
            {
                commands
                    .entity(entity)
                    .queue(|mut creature: bevy::ecs::world::EntityWorldMut| {
                        if let Some(mut mind) = creature.get_mut::<ai::Mind>() {
                            mind.queue_goes_on();
                        }
                    });
            }
        }
        if queue.0.is_empty() {
            commands.entity(entity).remove::<ScriptQueue>();
        }
    }
}

/// The creature that an `Attack` of the operands `ops` teleports, its first name, and where to,
/// its `0x04`'s cell, `DefPos` or creature's place (`0x4A2B40`, `ECS_TELEPORT`; #1831); none for
/// another art or none, for the hero, or without a place.
fn phased(
    ops: &[opensacred_data::netscript::Operand],
    places: &Places,
) -> Option<(String, Whither)> {
    let teleports = ops.iter().any(|o| {
        o.tag == ART && matches!(&o.value, Value::Str(art) if art.eq_ignore_ascii_case(TELEPORTS))
    });
    if !teleports {
        return None;
    }
    let name =
        name_of(ops).filter(|name| !name.is_empty() && name != HERO && attack_finds(name))?;
    Some((name, whither(ops, 0x04, places)?))
}

/// Whether an `Attack` finds the creature of its attacker's name `name` (`0x4A2B40`, #1871): it
/// takes the name's key (`+0xA880`), the number of a `res:<digits>` or the [`sacred_key`] of the
/// text after `res:`, as `CreateNPC` keeps the creature by it, only where it is positive as a
/// signed number; else it looks the name up among the scripts' own names (`0x465070`), which hold
/// no creature, and the record does nothing (seen: quest 10252's boss, key `0xCF0288D9`). Of the
/// 20 trees' records, only 133 of `ECS_METEOR`'s 161 name such an attacker (`ENDGEGNER1` of `bum`,
/// `ENDGEGNER_10252_A`). A name of another form is taken as found.
///
/// [`sacred_key`]: opensacred_data::text::sacred_key
fn attack_finds(name: &str) -> bool {
    let Some(text) = name.strip_prefix("res:") else {
        return true;
    };
    let key = if !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) {
        text.parse::<u32>().unwrap_or(u32::MAX)
    } else {
        opensacred_data::text::sacred_key(text.as_bytes())
    };
    key.cast_signed() > 0
}

/// The spells that the scripts' `Attack`s name by their arts (#1871): `ECS_FIRESTRIKE` the
/// fireball, `ECS_WINDSTRIKE` the whirlwind, `ECS_METEOR` the meteors and `ECS_FLASH` the chain
/// lightning (#1844's count of the 20 trees: 16, 9, 161 and 4 records).
const SCRIPT_SPELLS: [(&str, u16); 4] = [
    ("ecs_firestrike", 5),
    ("ecs_windstrike", 29),
    ("ecs_meteor", 26),
    ("ecs_flash", 33),
];

/// Whom or where a script's cast is at (#1871): the hero, a creature of the name, a member of the
/// group, or a place.
#[derive(Clone, Debug, PartialEq)]
enum CastAt {
    Hero,
    Named(String),
    Group(u32),
    Place(Whither),
}

/// The creature that an `Attack` of the operands `ops` orders to cast (`0x4A2B40`, the event
/// `0x105`; #1871), its first name, the spell of its art, and its target: the next name (the hero
/// for `HERO`), else a group (`0x11`), else a place (`0x04`: a cell, a `DefPos` of `places` or a
/// creature's place), else the hero; none for another art or none, or for the hero as the caster.
fn cast_by(
    ops: &[opensacred_data::netscript::Operand],
    places: &Places,
) -> Option<(String, u16, CastAt)> {
    let spell = ops.iter().find_map(|o| match (o.tag, &o.value) {
        (ART, Value::Str(art)) => SCRIPT_SPELLS
            .iter()
            .find(|(name, _)| art.eq_ignore_ascii_case(name))
            .map(|&(_, spell)| spell),
        _ => None,
    })?;
    let mut names = ops.iter().filter_map(|o| match (o.tag, &o.value) {
        (0x01, Value::Str(name)) => Some(name.to_ascii_lowercase()),
        _ => None,
    });
    let caster = names
        .next()
        .filter(|name| !name.is_empty() && name != HERO && attack_finds(name))?;
    let group = ops.iter().find_map(|o| match (o.tag, &o.value) {
        (0x11, Value::I32(group)) => u32::try_from(*group).ok(),
        _ => None,
    });
    let at = match (names.next_back(), group, whither(ops, 0x04, places)) {
        (Some(name), ..) if name == HERO => CastAt::Hero,
        (Some(name), ..) => CastAt::Named(name),
        (None, Some(group), _) => CastAt::Group(group),
        (None, None, Some(place)) => CastAt::Place(place),
        (None, None, None) => CastAt::Hero,
    };
    Some((caster, spell, at))
}

/// The type that a `Morph` of the operands `ops` turns its creature into, its `0x02`; none for 0.
fn morphed_type(ops: &[opensacred_data::netscript::Operand]) -> Option<u32> {
    ops.iter()
        .find_map(|o| match (o.tag, &o.value) {
            (0x02, Value::I32(n)) => u32::try_from(*n).ok(),
            (0x02, Value::U16(n)) => Some(u32::from(*n)),
            _ => None,
        })
        .filter(|&creature| creature > 0)
}

/// A creature that a `Morph` turns into another type ([`morph`]; `0x4A15A0`, #1829), and what
/// becomes of its level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Morph {
    name: String,
    creature: u32,
    level: Morphed,
    /// The operands of the `SetNPCState`s of the same name after it in its run, which `npcs`
    /// applies to the placements before the `Morph` turns the creature, and which the creature of
    /// the new type takes again, its side by its new type among them.
    stated: Vec<Vec<opensacred_data::netscript::Operand>>,
}

/// The level of a creature that a `Morph` turns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Morphed {
    /// At once: the level's setter with the level 1 (`0x586C00`, `0x564D60(1, 0, 0)`), as a
    /// `CreateNPC`'s level tag of 1 makes it, its health whole.
    Reset,
    /// At once, a `SetNPCState` of the same run giving it a level after: that level.
    Stated,
    /// At its turn in its queue (the step `0x59F266`): its level and health kept.
    Kept,
}

/// The `Morph`s of `records` that act now ([`morph`]): those outside the anim mode, and those
/// whose turn came in their creature's queue ([`QUEUED_MORPH`]); a `Morph` of no name, of the hero
/// or of no type does nothing (`0x4A15A0`).
fn morphs(records: &[Record]) -> Vec<Morph> {
    let mut queued = false;
    let mut morphs = Vec::new();
    for (index, record) in records.iter().enumerate() {
        match record.kind {
            SET_ANIM_MODE | SET_CINEMA_MODE => queued = true,
            SET_PLAY_MODE => queued = false,
            _ => {}
        }
        let Body::Operands(ops) = &record.body else {
            continue;
        };
        if record.kind != MORPH {
            continue;
        }
        let (Some(name), Some(creature)) =
            (name_of(ops).filter(|name| name != HERO), morphed_type(ops))
        else {
            continue;
        };
        let turned = ops.iter().any(|o| o.tag == QUEUED_MORPH);
        if queued && !turned {
            continue;
        }
        let stated: Vec<Vec<opensacred_data::netscript::Operand>> = records[index + 1..]
            .iter()
            .filter(|later| later.kind == SET_NPC_STATE)
            .filter_map(|later| match &later.body {
                Body::Operands(ops) if name_of(ops).as_ref() == Some(&name) => Some(ops.clone()),
                _ => None,
            })
            .collect();
        let level = if turned {
            Morphed::Kept
        } else if stated.iter().any(|ops| level_tag(ops) > 0) {
            Morphed::Stated
        } else {
            Morphed::Reset
        };
        morphs.push(Morph {
            name,
            creature,
            level,
            stated,
        });
    }
    morphs
}

/// An `Attack`'s attacker or target (`0x4A2B40`): the hero (`HERO`, case aside), the creature of
/// a name, or the members of a scripts' group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Party {
    Hero,
    Named(String),
    Group(u32),
}

/// A script's `Attack` without an art, which `script_attacks` carries out (#1830): its attacker,
/// its first `0x01` or `0x11`, and its target, the next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ScriptAttack {
    pub attacker: Party,
    pub target: Party,
}

/// The `Attack`s without an art that the scripts ran outside the anim mode, in order.
#[derive(Resource, Default)]
pub(crate) struct ScriptAttacks(pub Vec<ScriptAttack>);

/// The `Attack`s of `records` that act at once (`0x4A2B40`): outside the anim mode, of no art
/// (#1831's), each of an attacker and a target in their operands' order (#1835): the first
/// `0x01` or `0x11` the attacker, the next the target, a later name the target again. The hero
/// never attacks (no record names it first); under the mode a creature's waits its turn and, of
/// no art, never fights ([`QUEUED_OTHERS`]).
fn script_attacks(records: &[Record]) -> Vec<ScriptAttack> {
    let mut queued = false;
    let mut attacks = Vec::new();
    for record in records {
        match record.kind {
            SET_ANIM_MODE | SET_CINEMA_MODE => queued = true,
            SET_PLAY_MODE => queued = false,
            _ => {}
        }
        let Body::Operands(ops) = &record.body else {
            continue;
        };
        if record.kind != ATTACK || queued {
            continue;
        }
        let art = ops.iter().any(|o| {
            o.tag == ART
                && !matches!(&o.value, Value::Str(name) if name.eq_ignore_ascii_case(NO_SPELL))
        });
        if art {
            continue;
        }
        let mut parties = Vec::new();
        for op in ops {
            let party = match (op.tag, &op.value) {
                (0x01, Value::Str(name)) if name.eq_ignore_ascii_case(HERO) => Party::Hero,
                (0x01, Value::Str(name)) => Party::Named(name.to_ascii_lowercase()),
                (0x11, Value::I32(group)) => match u32::try_from(*group) {
                    Ok(group) => Party::Group(group),
                    Err(_) => continue,
                },
                _ => continue,
            };
            if parties.len() < 2 {
                parties.push(party);
            } else if let Some(target) = parties.last_mut() {
                *target = party;
            }
        }
        if let [attacker, target] =
            <[Party; 2]>::try_from(parties).unwrap_or([Party::Hero, Party::Hero])
            && attacker != Party::Hero
            && !matches!(&attacker, Party::Named(name) if !attack_finds(name))
        {
            attacks.push(ScriptAttack { attacker, target });
        }
    }
    attacks
}

/// The `Morph`s that the scripts ran, which [`morph`] carries out.
#[derive(Resource, Default)]
pub(crate) struct Morphs(Vec<Morph>);

/// Carries out the scripts' `Morph`s (`0x464170`, #1829): each creature of the name in the world
/// is made again of the new type where it stands, on its floor, with its placement's side (a
/// `SetNPCState` of the same run already in it), its group, its dialogue, its death's functions
/// and its hire, a follower hired again by its master (#2839), its sector's still and its
/// placement changed with
/// it, as a `RegionChange` makes one (#1064); at once at the level that the level's setter gives
/// the level 1 (or the level that a `SetNPCState` after it gives), its health whole; at its turn
/// in its queue at its level and its share of health. The original keeps the same object, its
/// type and model swapped; the creature made again keeps what the game keeps of one
/// (docs/decisions/netscript.md, "Morph made again"), and what a run ordered the creature before
/// it, its scripts' cast and its queue's steps (#3704). The hero never changes.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn morph(
    mut commands: Commands,
    mut morphs: ResMut<Morphs>,
    mut npcs: ResMut<Npcs>,
    mut streamer: ResMut<Streamer>,
    (heights, states, assets): (Res<Heights>, Res<buildings::Buildings>, Res<AssetServer>),
    (experience, played, holding): (
        Res<fight::Experience>,
        Res<drops::Survival>,
        Query<&fight::Holding, With<hero::Body>>,
    ),
    creatures: Query<
        (
            Entity,
            (&ScriptName, &Transform, &crowd::InTheWay),
            (&fight::Health, &ai::Mind),
            (
                Option<&Placed>,
                Option<&Remakes>,
                Option<&OnDeath>,
                Option<&Talk>,
                (
                    Option<&crate::followers::Hired>,
                    Option<&crate::followers::Follower>,
                ),
                (
                    Option<&crate::creature_casts::ScriptCast>,
                    Option<&ScriptQueue>,
                ),
            ),
        ),
        Without<Dies>,
    >,
) {
    if morphs.0.is_empty() {
        return;
    }
    let npcs = &mut *npcs;
    let side = u32::try_from(SIDE).unwrap_or(64);
    let hero = experience.level;
    let time = played.time(holding.iter().next());
    for morphed in std::mem::take(&mut morphs.0) {
        for (
            entity,
            (named, at, way),
            (health, mind),
            (placed, remakes, on_death, talk, (hired, follower), (cast, queue)),
        ) in &creatures
        {
            if named.0 != morphed.name {
                continue;
            }
            let placement = match (placed.map(|placed| &placed.source), remakes) {
                (Some(Source::Kept(index)), _) => npcs.placements.get(*index).cloned(),
                (Some(Source::Filled(placement)), _) => Some((**placement).clone()),
                (None, Some(remakes)) => Some((*remakes.0).clone()),
                (None, None) => None,
            };
            let Some(placement) = placement else {
                bevy::log::debug!("npcs: {} has no placement to morph", morphed.name);
                continue;
            };
            let items = placement.items.clone();
            let mut placement =
                match npcs
                    .makers
                    .remade(&placement, morphed.creature, &items, &mut npcs.slots)
                {
                    Ok(Some(placement)) => placement,
                    Ok(None) => continue,
                    Err(e) => {
                        bevy::log::warn!("npcs: {e:#}");
                        continue;
                    }
                };
            placement.creature.0 = morphed.creature;
            if morphed.level == Morphed::Reset {
                placement.tag = 1;
            }
            for ops in &morphed.stated {
                State::of(ops, &npcs.dialogues).apply(&mut placement);
            }
            let (x, y) = paths::cell_of(at.translation.x, at.translation.z);
            let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
                continue;
            };
            if let Some(Source::Kept(index)) = placed.map(|placed| &placed.source)
                && let Some(kept) = npcs.placements.get_mut(*index)
            {
                *kept = placement.clone();
            }
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
            let sector = (x / side, y / side);
            let level = match morphed.level {
                Morphed::Kept => health.level,
                Morphed::Reset | Morphed::Stated => {
                    let values = npcs.values.get(&sector).copied().unwrap_or_default();
                    let level = npcs.scripted_level(&there, hero, time);
                    let big = there.creature.1 & crowd::BIG != 0;
                    there
                        .rank
                        .level(level, (hero, time), &values, big, &npcs.fight)
                }
            };
            bevy::log::info!(
                "npcs: the scripts morph {}, a {}, into a {} of level {level}",
                morphed.name,
                mind.creature,
                morphed.creature
            );
            let made = spawn(
                &mut commands,
                &there,
                &heights,
                &states,
                &assets,
                (level, &npcs.fight, &npcs.items),
            );
            if morphed.level == Morphed::Kept {
                commands
                    .entity(made)
                    .insert(health_kept(*health, &there, &npcs.fight));
            }
            if let Some(hired) = hired {
                commands.entity(made).insert(*hired);
            }
            // What a run ordered it before its `Morph`, a cast and its queue's steps, goes on with
            // the creature made again, as with the original's same object (`refresh_a`'s meteor,
            // #3704).
            if let Some(&cast) = cast {
                commands.entity(made).insert(cast);
            }
            if let Some(queue) = queue {
                commands.entity(made).insert(queue.clone());
            }
            // A follower stays its master's (#2839).
            if let Some(&follower) = follower {
                commands.queue(move |world: &mut bevy::prelude::World| {
                    crate::followers::rehire(
                        world,
                        made,
                        follower.master,
                        (follower.escorted(), follower.listed()),
                    );
                });
            }
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
            streamer.adopt(&mut commands, sector, made);
        }
    }
}

/// What an `ActObj` does to the trigger of the object `object`'s door, as a `SetObjState` would
/// (`0x4A4310`, the step `0x59E9D5`; #1827): opening, the trigger's bar (`0x8000`) cleared and its
/// open bit set, the door swinging open (the object's slot 170); closing, the open bit cleared, the
/// door swinging shut (171). A mount mounted or left (`0x4266F0`) is not followed.
fn act_on(object: &str, open: bool) -> crate::scripts::ObjState {
    crate::scripts::ObjState {
        name: object.to_owned(),
        barred: open.then_some(false),
        open: Some(open),
        signal: false,
        trigger: false,
    }
}

/// A queued `PlaySound`'s sound of the id (the step `0x59ED41`): `playSFX` (`0x693510`) in the
/// middle (the creature −1), at the rate's factor 0.5, its sample's own rate, and the volume 1,
/// not the line by name (#1826).
fn queued_sound(id: u32) -> sfx::Play {
    sfx::Play {
        sound: sfx::Of::Id(id),
        at: None,
        rate: (0.5, sfx::Roll::None),
    }
}

/// Stops the hero at each tick of the world while the scripts' cinema holds and its queue is
/// empty (`0x539613`, `0x54E6D0(1, 0)`, of a hero whose AI state is neither 0 nor 10; #1892): its
/// walk and its attack end, so that it stands through a cinema that a function leaves on, which
/// takes none of the player's clicks on the world either (`scripts::no_cinema`).
fn cinema_holds(
    mut commands: Commands,
    (tick, mode): (Res<WorldTick>, Option<Res<crate::scripts::Mode>>),
    mut heroes: Query<(Entity, &mut hero::Body, Has<crate::attack::Attack>), Without<ScriptQueue>>,
) {
    if tick.seconds.is_none() || !mode.is_some_and(|mode| mode.cinema) {
        return;
    }
    for (entity, mut body, attacks) in &mut heroes {
        if body.moving().is_some() || body.target().is_some() || body.ordered() {
            body.stop();
        }
        if attacks {
            commands.entity(entity).remove::<crate::attack::Attack>();
        }
    }
}

/// Where a queued `Partikel`'s effect goes: the scripts' generic systems and the casts' sights,
/// none where their messages are not kept (the tests of the queues alone).
type Particles<'w> = (
    Option<ResMut<'w, bevy::ecs::message::Messages<crate::scripts::Partikel>>>,
    Option<ResMut<'w, bevy::ecs::message::Messages<crate::cast_sights::ScriptSights>>>,
);

/// A queued `Partikel`'s effect at its turn (the step `0x59CE78`, #1890): of `n0` −2 the casts'
/// sights of its `n1` at `caster` (`opensacred_rules::particles::ritual_sights`, two objects 791),
/// else its generic system at the creature of its first name `at`, as the record makes one at
/// once (`scripts::Partikel::of`).
fn partikel_turn(
    (at, caster, numbers): (&str, Option<Entity>, &[i32]),
    (particles, sights): &mut Particles,
) {
    if numbers.first() == Some(&-2) {
        if let (Some(caster), Some(sights)) = (caster, sights.as_deref_mut()) {
            sights.write(crate::cast_sights::ScriptSights {
                caster,
                code: numbers.get(1).copied().unwrap_or(-1),
            });
        }
        return;
    }
    if let (Some(partikel), Some(particles)) = (
        crate::scripts::Partikel::of(&[at], numbers),
        particles.as_deref_mut(),
    ) {
        particles.write(partikel);
    }
}

/// Takes the first step of the hero's [`ScriptQueue`] at each tick of the world, as the hero's
/// update steps its queue as a creature's (`0x5375C0` calling `0x59A840` for the local hero, seen
/// in `annehmen2003`'s run, #1280): a walk orders the hero to its target at a run, the player's
/// gait set to it (`+0x1F8` 2, seen kept after), steers it while it goes (its course found again
/// from where it stands at each tick, [`hero::Body::steer`]), and ends once it stands
/// within 40 of the target along each axis on the walk's level, or once it was ordered more than
/// five times without the hero moving; a teleport puts it there as the scripts' `Teleport` puts a
/// player; a `PlaySound`'s sound, a `Partikel`'s effect and an `ActObj`'s door are done at their
/// turns as a creature's ([`take_steps`]). The Sacred campaign queues no other step for the hero.
/// The queue goes once empty.
fn take_hero_steps(
    mut commands: Commands,
    (tick, mut plays, mut states): (
        Res<WorldTick>,
        MessageWriter<sfx::Play>,
        Option<ResMut<crate::scripts::ObjStates>>,
    ),
    hero: Option<ResMut<hero::Hero>>,
    mut heroes: Query<(Entity, &mut ScriptQueue, &mut hero::Body, &mut Transform)>,
    mut particles: Particles,
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
                body.steer();
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
        Some(&mut Queued::Sound(id)) => {
            plays.write(queued_sound(id));
            true
        }
        Some(Queued::Partikel {
            at,
            caster,
            numbers,
        }) => {
            partikel_turn((at, *caster, numbers), &mut particles);
            true
        }
        Some(Queued::Obj { object, open }) => {
            if let Some(states) = states.as_deref_mut() {
                states.0.push(act_on(object, *open));
            }
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
/// [`Npcs::add`]), and carries out on the creatures and the objects in the world what their
/// other records do to them: their `EquipNPC`s, their `SetNPCState`s ([`Npcs::restate`]), their
/// `SetGroupState`s and `DelNPC`s ([`Npcs::regroup`]) and their moves ([`Npcs::moves`]), at once
/// or as steps of the creatures' queues under the anim mode ([`ScriptQueue`]); their `Morph`s
/// and their `Attack`s without an art it hands on ([`Morphs`], [`ScriptAttacks`]).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
pub(crate) fn script_records(
    mut commands: Commands,
    records: Option<ResMut<crate::scripts::WorldRecords>>,
    mut npcs: ResMut<Npcs>,
    (heights, heroes, mut bodies, streamer, mut changes, crowd): (
        Res<Heights>,
        Query<&Transform, With<hero::Body>>,
        Query<(Entity, &mut hero::Body)>,
        Res<Streamer>,
        ResMut<AreaChanges>,
        Option<Res<crowd::Crowd>>,
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
            Option<&Called>,
        ),
        (Without<Dies>, Without<hero::Body>),
    >,
    (mut queues, real, sounds, mut lines, mut states, mut morphed, mut attacks, mut pending): (
        Query<&mut ScriptQueue>,
        Res<Time<bevy::time::Real>>,
        Option<Res<sfx::Sfx>>,
        MessageWriter<crate::scripts::ScriptLine>,
        Option<ResMut<crate::scripts::ObjStates>>,
        ResMut<Morphs>,
        ResMut<ScriptAttacks>,
        ResMut<crate::creature_casts::PendingCasts>,
    ),
    (dying, mut particles, traders, objects, indexed, giving, buried): (
        Query<
            (
                Entity,
                &ScriptName,
                &Transform,
                Option<&crowd::InTheWay>,
                Option<&Talk>,
                Option<&Corpse>,
                Option<&Stashed>,
            ),
            With<Dies>,
        >,
        Particles,
        Query<&Trades>,
        Query<(Entity, &ObjectName)>,
        Query<(Entity, &PlacementIndex), Without<Dies>>,
        (
            Res<fight::Experience>,
            Option<Res<crate::game::Difficulty>>,
            Option<Res<drops::Tables>>,
            Option<ResMut<crate::pickup::Pack>>,
            MessageWriter<crate::pickup::Taken>,
        ),
        Query<(Entity, &Stashed), With<Dies>>,
    ),
) {
    let (experience, difficulty, items, mut pack, mut taken) = giving;
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
    // The dead that the scripts name, whose queues take the steps of the anim mode as the
    // living's do (`0x59A840`, #2477): Anducar's of `QIS_OnEnter56`, whose `PlayAnim` lifts him.
    let dead: Vec<(Entity, String)> = dying
        .iter()
        .map(|(entity, named, ..)| (entity, named.0.clone()))
        .collect();
    let dead_of = |name: &str| {
        dead.iter()
            .filter(move |(_, named)| named == name)
            .map(|&(entity, _)| entity)
            .collect::<Vec<_>>()
    };
    // The `CreateObj`s that wait their turns in the queues of the creatures they name, under the
    // anim mode, which the moves below queue (#1826): the others made at once.
    let present: HashSet<&str> = creatures
        .iter()
        .filter_map(|(_, named, ..)| named.map(|named| named.0.as_str()))
        .collect();
    let deferred = deferred_objects(&records, |name| present.contains(name));
    let all = (!deferred.is_empty()).then(|| records.clone());
    if !deferred.is_empty() {
        let mut index = 0;
        records.retain(|_| {
            index += 1;
            !deferred.contains(&(index - 1))
        });
    }
    // The places of the creatures that the records name (`CPOS:`), as they stand now: their
    // cells and floors (#1046); for none, the hero's, or the first free point about the hero
    // where another creature stands in its cell (`0x45FF80`, #2858).
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
            let (hero, level) = bodies
                .iter()
                .next()
                .map_or((Entity::PLACEHOLDER, None), |(hero, body)| {
                    (hero, body.level())
                });
            let ((x, y), floor) = crate::paths::about_hero(
                (hero, at.translation, level),
                (&heights, crowd.as_deref()),
                true,
            );
            (x, y, floor)
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
                    .find(|(_, named, ..)| named.0 == name)
                    .map(|(_, _, at, way, ..)| {
                        cell_at(at.translation, way.map_or(0, |way| floor_of(way.level)))
                    })
            }),
    });
    let (born, equipped, into_pack) = match npcs.add(&records, |at| streamer.area(at)) {
        Ok(added) => {
            if added.placed > 0 {
                bevy::log::info!(
                    "npcs: the scripts place {} creatures and objects",
                    added.placed
                );
            }
            changes.0.extend(added.changes);
            (added.born, added.equipped, added.into_pack)
        }
        Err(e) => {
            bevy::log::warn!("npcs: the scripts' records: {e:#}");
            (Vec::new(), Vec::new(), Vec::new())
        }
    };
    // A `CreateObj` of the tag `0x3F` gives its item straight into the hero's backpack where it
    // finds room, marked until it is moved, the scripts hearing of it as of an item picked up into
    // the grid, its placement taken; a key goes to the keys' list, which no trigger asks, its
    // object gone; without room it lies where its record put it, unmarked (`0x48862D` to
    // `0x48873E`, #3543).
    let laying = items
        .as_deref()
        .map(|items| (items, difficulty.map(|d| d.0).unwrap_or_default()));
    for index in into_pack {
        let Some((dropped, uses)) = npcs.pack_item(index, (experience.level, laying)) else {
            continue;
        };
        let item = dropped.item;
        let size = items.as_deref().map_or((1, 1), |items| items.size(item));
        if items
            .as_deref()
            .is_some_and(|items| items.category(item) == KEY)
        {
            bevy::log::info!("npcs: the scripts give the key {item} to the hero's keys");
            npcs.taken.insert(index);
        } else if pack
            .as_deref_mut()
            .is_some_and(|pack| pack.give_marked(dropped, size, uses))
        {
            bevy::log::info!("npcs: the scripts give {item} straight into the hero's backpack");
            npcs.taken.insert(index);
            taken.write(crate::pickup::Taken(item));
        } else {
            bevy::log::info!(
                "npcs: no room in the hero's backpack for {item}, which lies where its record put it"
            );
        }
    }
    // Kept for a save, whose load places them again (`Npcs::resume`, #1958).
    if !records.is_empty() {
        npcs.records.push(records.clone());
    }
    let icons = set_icons(&records);
    let npcs = &mut *npcs;
    // An `EquipNPC` of a creature in the world that earlier records made puts its items on it
    // (`0x481D80`, #2756): what it holds made anew, those it held keeping their values, and what
    // it wears that it did not put on.
    for (index, before, after) in equipped {
        let Some(placement) = npcs.placements.get(index) else {
            continue;
        };
        let creature = placement.creature.0;
        let worn: Vec<u32> = npcs
            .makers
            .arming(&before, creature)
            .worn
            .iter()
            .map(|&(item, _)| item)
            .collect();
        let arming = npcs.makers.arming(&after, creature);
        for (entity, _) in indexed.iter().filter(|(_, at)| at.0 == index) {
            bevy::log::info!("npcs: the scripts equip {entity} with {after:?}");
            commands
                .entity(entity)
                .insert(Rearms::equipping(&arming, &worn));
        }
    }
    reicon(&npcs.dialogues, &mut npcs.icons, icons);
    for (name, state) in npcs.restate(&records) {
        // A dead creature takes its dialogue too, which holds its body to be spoken to (the dying
        // action's hold, `0x53ECA0`; #2444): Wilbur's of quest 45, given by his death's function.
        if state.talk.is_some() {
            for (entity, .., talk, _, _) in dying.iter().filter(|(_, named, ..)| named.0 == name) {
                let talk = state.talk_of(talk.map(|talk| talk.0.clone()));
                talk_to(&mut commands, entity, talk);
                bevy::log::info!("npcs: the scripts set {name}'s dialogue as it lies dead");
            }
        }
        // A dead creature's body let go (`0x80`, `0x46224D`–`0x4622B6`, #3445): its hold cleared,
        // its talk taken away and its stamp put back, so that it vanishes a second later.
        if state.releases {
            for (entity, ..) in dying.iter().filter(|(_, named, ..)| named.0 == name) {
                talk_to(&mut commands, entity, None);
                commands.entity(entity).insert(crate::bodies::Released);
                bevy::log::info!("npcs: the scripts let {name}'s body go");
            }
        }
        // White butterflies over its head for its seconds, living or dying (`0x7F`, `0x462180`,
        // #3446): quest 2101's deaths' functions show them over their bodies.
        if let Some(seconds) = state.butterflies {
            let named: Vec<Entity> = dying
                .iter()
                .filter(|(_, named, ..)| named.0 == name)
                .map(|(entity, ..)| entity)
                .chain(
                    creatures
                        .iter()
                        .filter(|(_, named, ..)| named.is_some_and(|named| named.0 == name))
                        .map(|(entity, ..)| entity),
                )
                .collect();
            for entity in named {
                crate::knockouts::white(&mut commands, entity, seconds);
            }
        }
        // A dead creature takes a vision's mark too (#3064): the Underworld's `Game_over` marks
        // Vilya as the same run's `SetHP` lifts her.
        if state.vision {
            for (entity, corpse) in dying
                .iter()
                .filter(|(_, named, ..)| named.0 == name)
                .filter_map(|(entity, .., corpse, _)| Some((entity, corpse?)))
            {
                envision(commands.entity(entity), corpse.creature);
            }
        }
        for (entity, _, on_death, mut transform, mut mind, way, _, talk, ..) in creatures
            .iter_mut()
            .filter(|(_, named, ..)| named.is_some_and(|named| named.0 == name))
        {
            if let Some(hire) = state.hire {
                commands
                    .entity(entity)
                    .insert((crate::followers::Hired(hire), crate::followers::ByState));
            }
            // Its level, 1000 a boss (#2966).
            if state.level > 0 {
                let tag = state.level;
                commands.queue(move |world: &mut bevy::prelude::World| {
                    state_level(world, entity, tag);
                });
            }
            match state.rises {
                Some(true) => {
                    commands.entity(entity).insert(RisesAgain);
                }
                Some(false) => {
                    commands.entity(entity).remove::<RisesAgain>();
                }
                None => {}
            }
            if state.talk.is_some() || state.stocked {
                let talk = state.talk_of(talk.map(|talk| talk.0.clone()));
                if let Some(mind) = mind.as_mut() {
                    mind.dialogue = talk.is_some();
                }
                talk_to(&mut commands, entity, talk);
            }
            if state.sights()
                && let Some(mind) = mind.as_mut()
            {
                mind.flags |= minds::SIGHT;
            }
            // A vision's mark, which it keeps (#3064): Vilya in the Underworld's last scene.
            if state.vision
                && let Some(mind) = mind.as_ref()
            {
                envision(commands.entity(entity), mind.creature);
            }
            // Its body let go before its death (`0x80`, #3445): no hold of the flags word, no
            // talk, its body then going at its time after its death.
            if state.releases {
                if let Some(mind) = mind.as_mut() {
                    mind.flags &= !opensacred_rules::undead::LIES_ON;
                    mind.dialogue = false;
                }
                talk_to(&mut commands, entity, None);
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
                if let Some(trader) = &state.stock {
                    commands.entity(entity).insert(crate::stables::Stabled {
                        trader: trader.clone(),
                    });
                }
                // Its flags word, as its placement's (`0x461540`, #2426): the arena's dark
                // gladiator takes the bit 1 by which the hero's look wakes it to fight; a
                // trader's mode `0x800` takes its trades.
                let trades = traders.get(entity).map_or(0, |trades| trades.0);
                let (flags, stated, untrades) = state.flags_of(mind.flags, trades);
                mind.flags = flags;
                // A word of the bit 0 written wakes it (`0x461DE6`, #3149).
                if stated & 0x1 != 0 {
                    commands.entity(entity).insert(crate::ai::Woken);
                }
                if untrades {
                    commands.entity(entity).remove::<Trades>();
                }
                if let Some(mut way) = way {
                    way.stands.hostile = ways::stops_the_hero(mind.flags);
                }
                // Its side by the record's hostility, as its placement's (`0x461540`, #1841).
                if let Some(hostile) = state.hostile {
                    mind.side = minds::stated_side(mind.side, mind.creature, stated, hostile);
                }
            }
            bevy::log::info!("npcs: the scripts set {name}'s state");
        }
    }
    for change in npcs.regroup(&records, &born) {
        // A `DelNPC` takes an item of its name out of the hero's slots too (`0x497F80`, #3147).
        if let Regrouped::Removed(name) = &change {
            let name = name.clone();
            commands.queue(move |world: &mut bevy::prelude::World| {
                crate::pickup::take_named(world, &name);
            });
        }
        for (entity, named, on_death, _, mind, way, health, _, _, called) in &mut creatures {
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
                    // A side changed flashes, its figure taken as it was (#2893).
                    if let Some(colour) = state
                        .hostile
                        .and_then(|hostile| minds::grouped_flash(mind.side, mind.creature, hostile))
                    {
                        commands.queue(move |world: &mut bevy::prelude::World| {
                            crate::group_looks::flash(world, entity, colour);
                        });
                    }
                    let mut functions = on_death.as_ref().map(|d| d.0.clone()).unwrap_or_default();
                    state.apply_to(&mut mind, way.map(|way| way.into_inner()), &mut functions);
                    talk_to(&mut commands, entity, state.talk.clone());
                    // Its key where it bears no name of its own, and its rising again
                    // (`0x49F0F7`, `0x6B`; #3148).
                    if let Some(key) = state.key
                        && named.is_none()
                        && called.is_none()
                    {
                        commands.entity(entity).insert(Called(key));
                    }
                    match state.rises {
                        Some(true) => {
                            commands.entity(entity).insert(RisesAgain);
                        }
                        Some(false) => {
                            commands.entity(entity).remove::<RisesAgain>();
                        }
                        None => {}
                    }
                    // Its level, 1000 a boss (#2894).
                    if state.level > 0 {
                        let tag = state.level;
                        commands.queue(move |world: &mut bevy::prelude::World| {
                            state_level(world, entity, tag);
                        });
                    }
                    match on_death {
                        Some(mut on_death) => on_death.0 = functions,
                        None if functions.is_empty() => {}
                        None => {
                            commands.entity(entity).insert(OnDeath(functions));
                        }
                    }
                }
                // Removed without a death, its function not run, nothing dropped: kept from
                // coming back as it goes (`sdk_DESTROY(id, 1, 0, 1)`, `0x463A70`); a group's
                // member in the soul's sparks, its figure taken before it goes (#2893), a
                // `DelNPC`'s with none.
                _ => {
                    if let Some(mut health) = health {
                        health.current = 0;
                    }
                    if matches!(&change, Regrouped::Group(state) if state.removes) {
                        commands.queue(move |world: &mut bevy::prelude::World| {
                            crate::group_looks::vanish(world, entity);
                        });
                    }
                    commands.entity(entity).despawn();
                }
            }
        }
        // A member dead already rises in the soul's sparks of its figure too, its body kept
        // (`0x49ED51`, #3148): its group its mind's, kept aside as it died.
        if let Regrouped::Group(state) = &change
            && state.removes
            && state.group.is_some()
        {
            for (dead, stashed) in &buried {
                if stashed.mind.group == state.group {
                    commands.queue(move |world: &mut bevy::prelude::World| {
                        crate::group_looks::vanish(world, dead);
                    });
                }
            }
        }
        match change {
            Regrouped::Group(state) => bevy::log::info!(
                "npcs: the scripts {} the group {}",
                if state.removes { "remove" } else { "set" },
                state.group.map_or(0, |group| group & !SCRIPT_GROUP)
            ),
            Regrouped::Removed(name) => {
                // The objects of the name too (`0x497F80`, #2758).
                for (object, _) in objects.iter().filter(|(_, named)| named.0 == *name) {
                    commands.entity(object).despawn();
                }
                bevy::log::info!("npcs: the scripts remove {name}");
            }
        }
    }
    morphed.0.extend(morphs(&records));
    attacks.0.extend(script_attacks(&records));
    // The steps queued in this run, for the creatures without a queue yet.
    let mut queued: HashMap<Entity, Vec<Queued>> = HashMap::new();
    for moved in npcs.moves(all.as_deref().unwrap_or(&records), real.elapsed_secs_f64()) {
        let (whither, (walks, strolls)) = match moved.act {
            Act::Go {
                whither,
                walks,
                strolls,
            } => (whither, (walks, strolls)),
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
                if !moved.queued {
                    // Outside the mode, the event `0x108`, which the creature's receive takes
                    // for the type 700 alone, Shaddar, dead or alive: the clip and its loops
                    // (`0x52E8AB`, #3252); a dead one is lifted to play it, as a queued clip
                    // lifts it.
                    for (entity, ..) in creatures.iter().filter(|(_, named, _, _, mind, ..)| {
                        named.is_some_and(|named| named.0 == moved.name)
                            && mind
                                .as_ref()
                                .is_some_and(|mind| clip_outside_the_mode(mind.thinks.creature))
                    }) {
                        commands
                            .entity(entity)
                            .remove::<Walk>()
                            .insert(ScriptClip::new(slot, loops));
                    }
                    for (entity, stashed) in
                        dying.iter().filter_map(|(entity, named, .., stashed)| {
                            let stashed = stashed?;
                            (named.0 == moved.name
                                && clip_outside_the_mode(stashed.mind.thinks.creature))
                            .then_some((entity, stashed))
                        })
                    {
                        lift(&mut commands, entity, stashed);
                        commands.entity(entity).insert(ScriptClip::new(slot, loops));
                    }
                    bevy::log::info!("npcs: the scripts play the slot {slot} of {}", moved.name);
                    continue;
                }
                let mut bearers: Vec<Entity> = creatures
                    .iter()
                    .filter(|(_, named, ..)| named.is_some_and(|named| named.0 == moved.name))
                    .map(|(entity, ..)| entity)
                    .collect();
                bearers.extend(dead_of(&moved.name));
                for entity in bearers {
                    let step = Queued::Clip { slot };
                    match queues.get_mut(entity) {
                        Ok(mut queue) => queue.0.push_back(step),
                        Err(_) => queued.entry(entity).or_default().push(step),
                    }
                }
                bevy::log::info!(
                    "npcs: the scripts play the slot {slot} of {}{}",
                    moved.name,
                    if moved.queued { " in its turn" } else { "" }
                );
                continue;
            }
            // A sound of no id plays nothing (`0x676170`); the hero's waits in its queue, a
            // creature's in each of its name's, and one of a name that none bears plays at once
            // as the line by name (`0x4A9730`).
            Act::Sound(sound) => {
                let Some((id, _)) = sounds
                    .as_ref()
                    .and_then(|sounds| sounds.named(&crate::talk::sound_name(&sound)))
                else {
                    continue;
                };
                let takers: Vec<Entity> = if moved.name == HERO {
                    bodies.iter().map(|(entity, _)| entity).collect()
                } else {
                    creatures
                        .iter()
                        .filter(|(_, named, ..)| named.is_some_and(|named| named.0 == moved.name))
                        .map(|(entity, ..)| entity)
                        .chain(dead_of(&moved.name))
                        .collect()
                };
                if takers.is_empty() {
                    lines.write(crate::scripts::ScriptLine(sound));
                    continue;
                }
                for entity in takers {
                    match queues.get_mut(entity) {
                        Ok(mut queue) => queue.0.push_back(Queued::Sound(id)),
                        Err(_) => queued.entry(entity).or_default().push(Queued::Sound(id)),
                    }
                }
                bevy::log::info!("npcs: the scripts play {sound} at {}'s turn", moved.name);
                continue;
            }
            // In the queue of the first creature of its second name in the world, the hero's for
            // `HERO`, its effect at its first name's creature as the record runs (`0x464BD0`,
            // #1890); made at once where the second is none.
            Act::Partikel { at, numbers } => {
                let find = |name: &str| {
                    if name == HERO {
                        bodies.iter().map(|(entity, _)| entity).next()
                    } else {
                        creatures
                            .iter()
                            .find(|(_, named, ..)| named.is_some_and(|named| named.0 == name))
                            .map(|(entity, ..)| entity)
                            .or_else(|| dead_of(name).first().copied())
                    }
                };
                let Some(taker) = find(&moved.name) else {
                    partikel_turn((&at, None, &numbers), &mut particles);
                    continue;
                };
                let step = Queued::Partikel {
                    caster: find(&at),
                    at,
                    numbers,
                };
                match queues.get_mut(taker) {
                    Ok(mut queue) => queue.0.push_back(step),
                    Err(_) => queued.entry(taker).or_default().push(step),
                }
                bevy::log::info!("npcs: a Partikel waits for {}'s turn", moved.name);
                continue;
            }
            // The hero's or a creature's in the world (`0x464BD0`), none else; under the mode at
            // its turn, in the queue of the first of its name.
            Act::Obj { object, open } => {
                let actor = if moved.name == HERO {
                    bodies.iter().map(|(entity, _)| entity).next()
                } else {
                    creatures
                        .iter()
                        .find(|(_, named, ..)| named.is_some_and(|named| named.0 == moved.name))
                        .map(|(entity, ..)| entity)
                        .or_else(|| dead_of(&moved.name).first().copied())
                };
                let Some(actor) = actor else {
                    continue;
                };
                bevy::log::info!(
                    "npcs: {} {} the door of {object}{}",
                    moved.name,
                    if open { "opens" } else { "closes" },
                    if moved.queued { " in its turn" } else { "" }
                );
                if moved.queued {
                    let step = Queued::Obj { object, open };
                    match queues.get_mut(actor) {
                        Ok(mut queue) => queue.0.push_back(step),
                        Err(_) => queued.entry(actor).or_default().push(step),
                    }
                } else if let Some(states) = states.as_deref_mut() {
                    states.0.push(act_on(&object, open));
                }
                continue;
            }
            Act::Phase(whither) => {
                let cell = match &whither {
                    Whither::Cell(cell) => Some(*cell),
                    Whither::Creature(_) => None,
                };
                let to = match whither {
                    Whither::Cell((x, y, layer)) => {
                        crate::script_attacks::Phase::to_cell((x, y, layer), &heights)
                    }
                    Whither::Creature(None) => heroes
                        .iter()
                        .next()
                        .map(|at| crate::script_attacks::Phase::to(at.translation, None)),
                    Whither::Creature(Some(name)) => creatures
                        .iter()
                        .find(|(_, named, ..)| named.is_some_and(|named| named.0 == name))
                        .map(|(_, _, _, at, _, way, ..)| {
                            crate::script_attacks::Phase::to(
                                at.translation,
                                way.and_then(|way| way.level),
                            )
                        }),
                };
                let Some(phase) = to else {
                    continue;
                };
                if let Some((entity, ..)) = creatures
                    .iter()
                    .find(|(_, named, ..)| named.is_some_and(|named| named.0 == moved.name))
                {
                    if moved.queued {
                        let step = Queued::Phase {
                            phase,
                            begun: false,
                        };
                        match queues.get_mut(entity) {
                            Ok(mut queue) => queue.0.push_back(step),
                            Err(_) => queued.entry(entity).or_default().push(step),
                        }
                    } else {
                        commands.entity(entity).insert(phase);
                    }
                    bevy::log::info!(
                        "npcs: the scripts teleport {}{}",
                        moved.name,
                        if moved.queued { " in its turn" } else { "" }
                    );
                } else if let Some((x, y, layer)) = cell {
                    // One away from the world goes there all the same, which its sector places
                    // as it comes in, as a `Teleport` puts it (#2448, #2470): the Underworld's
                    // quest 11 sends Valor's ghost from his throne to the hero. A cell of
                    // negative numbers is the cell `|c| − 1`, on the ground (`0x522742`).
                    let unsigned = |c: i32| if c < 0 { -c - 1 } else { c };
                    let layer = if x < 0 || y < 0 { 0 } else { layer };
                    if npcs.put_away(&moved.name, (unsigned(x), unsigned(y)), layer) {
                        bevy::log::info!(
                            "npcs: the scripts teleport {} to {},{}, away from the world",
                            moved.name,
                            unsigned(x),
                            unsigned(y)
                        );
                    }
                }
                continue;
            }
            Act::Cast { spell, ref at } => {
                // By the first creature of the name, at its target's creature, a member of its
                // group, the hero or a place, at once or at its turn (#1871).
                let named = |name: &str| {
                    creatures
                        .iter()
                        .find(|(_, named, ..)| named.is_some_and(|named| named.0 == name))
                        .map(|(entity, _, _, at, ..)| (entity, at.translation))
                };
                let hero = || {
                    bodies
                        .iter()
                        .next()
                        .map(|(entity, _)| entity)
                        .zip(heroes.iter().next().map(|at| at.translation))
                };
                let target = match at {
                    CastAt::Hero => hero(),
                    CastAt::Named(name) => named(name),
                    CastAt::Group(group) => creatures
                        .iter()
                        .find(|(_, _, _, _, mind, ..)| {
                            mind.as_ref()
                                .is_some_and(|mind| mind.group == Some(SCRIPT_GROUP | group))
                        })
                        .map(|(entity, _, _, at, ..)| (entity, at.translation)),
                    CastAt::Place(Whither::Cell((x, y, _))) => {
                        let centre = crate::paths::centre((*x, *y));
                        let height = heights.standing(centre.x, centre.y, None).unwrap_or(0.0);
                        Some((Entity::PLACEHOLDER, Vec3::new(centre.x, height, centre.y)))
                    }
                    CastAt::Place(Whither::Creature(None)) => hero(),
                    CastAt::Place(Whither::Creature(Some(name))) => named(name),
                };
                let Some((target, point)) = target else {
                    continue;
                };
                let cast = crate::creature_casts::ScriptCast::new(spell, target, point);
                // A caster that the run makes comes in a later frame: its cast waits for it.
                let Some((caster, _)) = named(&moved.name) else {
                    if !moved.queued {
                        pending.wait(moved.name.clone(), cast);
                    }
                    continue;
                };
                if moved.queued {
                    let step = Queued::Cast { cast, begun: false };
                    match queues.get_mut(caster) {
                        Ok(mut queue) => queue.0.push_back(step),
                        Err(_) => queued.entry(caster).or_default().push(step),
                    }
                } else {
                    commands.entity(caster).insert(cast);
                }
                bevy::log::info!(
                    "npcs: the scripts have {} cast {spell}{}",
                    moved.name,
                    if moved.queued { " in its turn" } else { "" }
                );
                continue;
            }
            Act::Morph(creature) => {
                let bearers: Vec<Entity> = creatures
                    .iter()
                    .filter(|(_, named, ..)| named.is_some_and(|named| named.0 == moved.name))
                    .map(|(entity, ..)| entity)
                    .chain(dead_of(&moved.name))
                    .collect();
                for entity in bearers {
                    let step = Queued::Morph {
                        name: moved.name.clone(),
                        creature,
                    };
                    match queues.get_mut(entity) {
                        Ok(mut queue) => queue.0.push_back(step),
                        Err(_) => queued.entry(entity).or_default().push(step),
                    }
                }
                continue;
            }
            // Made at the turn of the first creature of the name; where none bears it, it was
            // made at once.
            Act::Make(record) => {
                if let Some(entity) = creatures
                    .iter()
                    .find(|(_, named, ..)| named.is_some_and(|named| named.0 == moved.name))
                    .map(|(entity, ..)| entity)
                    .or_else(|| dead_of(&moved.name).first().copied())
                {
                    let step = Queued::Make(record);
                    match queues.get_mut(entity) {
                        Ok(mut queue) => queue.0.push_back(step),
                        Err(_) => queued.entry(entity).or_default().push(step),
                    }
                    bevy::log::info!("npcs: the scripts make an object at {}'s turn", moved.name);
                }
                continue;
            }
            act @ (Act::Other(_) | Act::Call(_) | Act::Wait(..)) => {
                let bearers: Vec<Entity> = creatures
                    .iter()
                    .filter(|(_, named, ..)| named.is_some_and(|named| named.0 == moved.name))
                    .map(|(entity, ..)| entity)
                    .chain(dead_of(&moved.name))
                    .collect();
                for entity in bearers {
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
                        orders: u8::from(strolls),
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
        let mut in_world = false;
        for (entity, _, _, mut transform, mind, way, _, _, layered, _) in creatures
            .iter_mut()
            .filter(|(_, named, ..)| named.is_some_and(|named| named.0 == moved.name))
        {
            in_world = true;
            // Under the anim mode, a step of its queue, in its turn (#1220): a walk to a cell
            // of no floor onto the creature's own level (`0x49E210`), a teleport's floor 0 the
            // ground.
            if moved.queued {
                let step = if walks {
                    Queued::Walk {
                        to,
                        onto: (layer > 0).then_some(level),
                        orders: u8::from(strolls),
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
                // At its walk speed, as the start's `NPC_Goto`s walk theirs (#132); to a cell of
                // the floor 0 onto its own level as the record runs (`0x49E210`, #2481).
                let speed = mind.as_ref().map_or(0.0, |mind| mind.speed);
                let from = way.as_ref().and_then(|way| way.level);
                let onto = if layer > 0 { level } else { from };
                commands
                    .entity(entity)
                    .insert(Walk::to(to, from, speed).onto(onto).idle());
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
        // A teleport of one away from the world puts it there all the same, which its sector
        // places as it comes in (#2448): Prince Valor into Tyr-Fasul's fortress, quest 54.
        let away =
            !in_world && !walks && npcs.put_away(&moved.name, paths::cell_of(to.x, to.z), layer);
        bevy::log::info!(
            "npcs: the scripts {} {} to {:?}{}{}",
            if walks { "send" } else { "put" },
            moved.name,
            whither,
            if moved.queued { " in its turn" } else { "" },
            if away { ", away from the world" } else { "" }
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
/// (docs/decisions/netscript.md, "Object states kept for a gone object's name"). A state of
/// the tag `0x3C` signals the FX objects of the name (`0x48B8C4`; `forge_elements`).
fn script_states(
    mut commands: Commands,
    states: Option<ResMut<crate::scripts::ObjStates>>,
    mut signals: MessageWriter<crate::forge_elements::Signal>,
    mut npcs: ResMut<Npcs>,
    mut heights: ResMut<Heights>,
    doors: Query<(Entity, &Door)>,
) {
    let Some(mut states) = states else {
        return;
    };
    for state in std::mem::take(&mut states.0) {
        if state.signal {
            signals.write(crate::forge_elements::Signal(state.name.clone()));
        }
        if state.barred.is_none() && state.open.is_none() {
            continue;
        }
        // A `SetTriggerState` names the trigger itself (#2450), a `SetObjState` its object.
        let trigger = if state.trigger {
            Some(state.name.clone())
        } else {
            npcs.named.get(&state.name).cloned()
        };
        let Some(trigger) = trigger else {
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
    /// Whether a creature killed it, not the hero: it drops nothing (`0x533BA8`, #630), unless
    /// that creature is the hero's follower ([`Self::by_follower`]).
    pub by_creature: bool,
    /// Whether the creature that killed it follows the hero, hired or summoned, of the hero's
    /// list `+0x39C` (`0x4FC900`): it drops, as no player's kill (`drops`; seen in the original,
    /// #2804).
    pub by_follower: bool,
    /// Whether anyone killed it: a `SetHP` of 0, a summoned follower replaced or a lifetime's
    /// end kills nobody's, which the scripts' kills do not count (`0x534F60`, #1021).
    pub killed: bool,
    /// Whether it is an undead that rises again (`undead`, #1199): its death drops nothing,
    /// gives no experience and is no kill (`0x533C7F`, `0x534775`).
    pub rises: bool,
    /// Whether it died before and lies as it comes, a body that its dialogue holds put back with
    /// its sector or a save (#2457): it dies no more, its death unspoken, running no death's
    /// function, and lies as it fell at once.
    pub fallen: bool,
    /// How long it has lain already, in seconds, which its [`Stashed`] counts on from: a save's
    /// follower that lay to rise again (#2947).
    lain: f32,
}

impl Dies {
    pub fn new(choice: u32) -> Self {
        Self {
            choice,
            played: false,
            by_creature: false,
            by_follower: false,
            killed: true,
            rises: false,
            fallen: false,
            lain: 0.0,
        }
    }

    /// A body that lies as it comes ([`Self::fallen`]).
    pub fn fallen() -> Self {
        Self {
            fallen: true,
            ..Self::unkilled()
        }
    }

    /// A body that lies as it comes ([`Self::fallen`]) and has lain `lain` seconds already: a
    /// save's follower that lay to rise again, which rises once it has lain its whole time
    /// (`followers::rise_again`, #2947).
    pub fn lying(lain: f32) -> Self {
        Self {
            lain,
            ..Self::fallen()
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

/// What a dead NPC was, which the Vampiress's awakening (1034) and a black magician's raising
/// (#1856) raise: its type, its `Creature.pak` class, whether it had a dialogue, the level it lies
/// on, whether it was hostile to the hero, and its flags word, whose `0x2000` holds it lying
/// (`opensacred_rules::undead::LIES_ON`, `bodies`).
#[derive(Component, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Corpse {
    pub creature: u32,
    pub class: u8,
    pub dialogue: bool,
    pub level: Level,
    pub hostile: bool,
    pub flags: u32,
}

/// A creature whose `+0x2B7` bit 8 a script's `0x6B` above 0 set ([`RISES`]): a follower of the
/// hero rises 20 s after its death (`0x53FB40`, `followers::rise_again`, #2459), and the strikes
/// leave one that follows no hero, but where a hero's strike clears the mark
/// (`fight::marked_after`, #3294).
#[derive(Component, Clone, Copy, Debug)]
pub struct RisesAgain;

/// A vision (the object's `+0x14` bit `0x200000`; docs/re/drawing.md, "The visions"): a creature
/// of the types `opensacred_rules::spells::VISIONS`, or one that a script's [`VISION`] marks
/// (#3064). It is drawn only in its look for as long as it lives (`skins::Overlay::Vision`), and
/// its body ends at once in sparks (`bodies`).
#[derive(Component, Clone, Copy, Debug)]
pub struct Vision;

/// Marks `entity`, a creature of the type `creature`, a vision ([`Vision`]) and gives it its look
/// for good (`skins`, #2519).
fn envision(mut entity: bevy::ecs::system::EntityCommands, creature: u32) {
    let look = opensacred_rules::spells::vision(creature);
    entity.insert((
        Vision,
        crate::skins::Asked(crate::skins::Overlay::Vision(look), f32::INFINITY),
    ));
}

/// The id of the text a fill's creature is called by, its `MakeGroup` rule's name, which
/// `FillSector` puts in the creature's key `+0x3C` ([`called_of`], #3071).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Called(pub u32);

/// The name of a creature of the type `creature` (`sdk_REFKEY`, `0x44A1C0`): the text of its
/// script's name `script`, else of the name its rule called it by (`called`), else its type's;
/// none where none has a text.
pub fn name_of_creature<'a>(
    texts: &'a crate::lines::Texts,
    (script, called): (Option<&ScriptName>, Option<&Called>),
    creature: u32,
) -> Option<&'a str> {
    script
        .and_then(|key| texts.named(crate::talk::resource(&key.0)))
        .or_else(|| called.and_then(|called| texts.id(called.0)))
        .or_else(|| texts.named(&creature.to_string()))
}

/// What a dead creature laid down as it died, which whatever lifts or raises it takes up again: a
/// `SetHP` or a queue of the scripts ([`lift`], #2477), its own rising again ([`RisesAgain`]),
/// the Vampiress's awakening (`followers::raise`, #3140): its mind and its place in the way, and
/// how long it has lain, in seconds.
#[derive(Component, Clone, Debug)]
pub struct Stashed {
    pub mind: ai::Mind,
    pub way: crowd::InTheWay,
    pub lain: f32,
}

/// Lays each dying NPC down: its dying clip once, its walk, its strike, its mind and its place in
/// the way gone, what it was kept ([`Corpse`]), its death's stamp put back where it dies beside a
/// soulcatcher ([`crate::bodies::PutBack`]), and its mind and place kept aside for whatever lifts
/// or raises it ([`Stashed`]).
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
        (Option<&ai::Mind>, Option<&crowd::InTheWay>),
        (Option<&Transform>, Option<&arrows::Size>),
    )>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    catchers: Query<(&Transform, &hero::Body, &crate::effects::Effects), Without<Npc>>,
    (speed, settings, mut dice): (
        Res<hero::GameSpeed>,
        Option<Res<crate::settings::Settings>>,
        ResMut<DyingDice>,
    ),
) {
    let detail = crate::settings::detail(settings.as_deref());
    for (entity, npc, mut dies, (mind, way), (at, size)) in &mut dying {
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
                hostile: mind.hostile(),
                flags: mind.flags,
            });
            // Beside a soulcatcher's object 869 on its level, as its death is stamped
            // (`0x53E889`–`0x53E8F0`, #3387): every time counted from its death comes sooner.
            let size = size.map_or(arrows::size(0), |size| size.0);
            let beside = at.is_some_and(|at| {
                catchers.iter().any(|(catcher, body, effects)| {
                    effects.holds(opensacred_rules::arts::SOULCATCHER)
                        && body.level() == way.level
                        && opensacred_rules::undead::beside_soulcatcher(
                            (catcher.translation - at.translation).xz().length(),
                            size,
                        )
                })
            });
            if beside && !dies.fallen {
                let back = opensacred_rules::undead::soulcatcher_back(detail);
                bevy::log::debug!(
                    "npcs: {entity} dies beside a soulcatcher, its stamp {back} s back"
                );
                commands.entity(entity).insert(crate::bodies::PutBack(back));
            }
            // Kept aside for whatever lifts or raises it: a `SetHP` or a queue of the scripts
            // (#2477), its own rising again (#2459), the Vampiress's awakening, which raises the
            // corpse itself (#3140).
            commands.entity(entity).insert(Stashed {
                mind: mind.clone(),
                way: *way,
                lain: dies.lain,
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
        // Its dying clip rolled at its death (`0x53EF90`–`0x53F31D`, #3400); a body put back
        // lies by the one it fell by.
        if !dies.fallen
            && let Some(mind) = mind
        {
            let slots: Vec<u16> = graph.dying.iter().map(|&(slot, _)| slot).collect();
            dies.choice = dying_choice(mind.creature, &slots, mind.rides.is_some(), &mut dice.0);
        }
        if let Some(&(slot, node)) = graph
            .dying
            .get(usize::try_from(dies.choice).unwrap_or(0) % graph.dying.len().max(1))
        {
            bevy::log::debug!("npcs: {entity} dies by its slot {slot}");
            // A body put back lies as it fell: its clip run through in a frame.
            let (blend, speed) = if dies.fallen {
                (Duration::ZERO, FALLEN_SPEED)
            } else {
                (BLEND, speed.0)
            };
            transitions.play(&mut player, node, blend).set_speed(speed);
        }
        dies.played = true;
    }
}

/// Whether the dying clip that `dies` chose of `npc` has ended, as its `+0x14` bit `0x2000000`
/// clears once its clip has ended (docs/re/fight.md, "The bodies' end", #3082): at once where it
/// plays none.
pub(crate) fn dying_ended(npc: &Npc, dies: &Dies, players: &Query<&AnimationPlayer>) -> bool {
    let (Some(graph), Some(player)) = (&npc.graph, npc.player) else {
        return true;
    };
    let index = usize::try_from(dies.choice).unwrap_or(0) % graph.dying.len().max(1);
    let Some(&(_, node)) = graph.dying.get(index) else {
        return true;
    };
    players
        .get(player)
        .ok()
        .and_then(|player| player.animation(node))
        .is_none_or(bevy::animation::ActiveAnimation::is_finished)
}

/// An NPC that strikes at `target` (the attack action 3, `0x4E4CF0`): one of its attack's clips,
/// chosen by `choice`, once, which hits at its fractions (`0x4EA1F0`); or that shoots at it (the
/// action 5, `0x4DCB80`), the clip at its attack speed times its chain's factor (`+0x210`), whose
/// arrow leaves at its first fraction (`0x4DD90E`), or its bolts under the Seraphim's effects
/// ([`Strikes::bolts`]).
#[derive(Component)]
pub struct Strikes {
    target: Entity,
    choice: u32,
    /// A shot's factor of its chain, none for a strike.
    shot: Option<f32>,
    /// A shot's bolts in place of its arrow, by the effect 16 or 17 it holds (#2899): the bolt
    /// whose clip it plays, and the bolt it looses, none where it looses nothing (#3030).
    bolt: Option<opensacred_rules::arts::Bolt>,
    fires: Option<opensacred_rules::arts::Bolt>,
    /// The clip it plays, its length and the fractions at which it hits, once it plays.
    playing: Option<(AnimationNodeIndex, f32, [f32; 2])>,
    /// How many hits it has struck.
    struck: u8,
    /// Whether the strike begins its attack, the action's sub-state 0, rather than chaining.
    opens: bool,
    /// Whether a shot that plays no clip ([`BoltClip::Kept`]) has seen its start's frame, and the
    /// world's ticks since its bolts left.
    begun: bool,
    after: u8,
    /// Whether it is a loaded save's, its clip played again from its start with the hits it has
    /// struck (#3759), its start's gunfire not heard again.
    resumed: bool,
}

/// An NPC's strike as a save keeps it (#3759): its target by its name, the hits it has struck and
/// its shot's state; a load plays its clip again from its start, as the original's load plays the
/// slot in play anew, so that a strike saved before its hit hits once and one saved after it hits
/// no more (docs/re/saves.md, "What a load gives back of the doings").
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedStrikes {
    #[serde(with = "crate::names::required")]
    target: Entity,
    choice: u32,
    shot: Option<f32>,
    bolt: Option<opensacred_rules::arts::Bolt>,
    fires: Option<opensacred_rules::arts::Bolt>,
    struck: u8,
    opens: bool,
    begun: bool,
    after: u8,
}

impl MapEntities for SavedStrikes {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.target = mapper.get_mapped(self.target);
    }
}

impl crate::doings::Saves for Strikes {
    type Saved = SavedStrikes;

    fn saved(&self, _: &crate::doings::Moment) -> Option<SavedStrikes> {
        Some(SavedStrikes {
            target: self.target,
            choice: self.choice,
            shot: self.shot,
            bolt: self.bolt,
            fires: self.fires,
            struck: self.struck,
            opens: self.opens,
            begun: self.begun,
            after: self.after,
        })
    }

    fn again(saved: SavedStrikes, _: &crate::doings::Moment) -> Option<Self> {
        Some(Self {
            target: crate::names::found(Some(saved.target))?,
            choice: saved.choice,
            shot: saved.shot,
            bolt: saved.bolt,
            fires: saved.fires,
            playing: None,
            struck: saved.struck,
            opens: saved.opens,
            begun: saved.begun,
            after: saved.after,
            resumed: true,
        })
    }
}

impl Strikes {
    /// A strike at `target`, with the clip `choice` of its attack's.
    pub fn at(target: Entity, choice: u32) -> Self {
        Self {
            target,
            choice,
            shot: None,
            bolt: None,
            fires: None,
            playing: None,
            struck: 0,
            opens: false,
            begun: false,
            after: 0,
            resumed: false,
        }
    }

    /// The strike, `opens` where it begins its attack rather than chaining onto one.
    pub fn opening(self, opens: bool) -> Self {
        Self { opens, ..self }
    }

    /// A shot at `target`, with the clip `choice` of its attack's, at its attack speed times
    /// `chain`.
    pub fn shot(target: Entity, choice: u32, chain: f32) -> Self {
        Self {
            shot: Some(chain),
            ..Self::at(target, choice)
        }
    }

    /// A shot of the bolts `bolt`, where it holds the Seraphim's effect 16 or 17 (`0x4DCB80`,
    /// #2899): its bolt's own clip at its own rate where the model has it, else its stand-in or
    /// none ([`BoltClip`]), and the bolts at its first fraction in place of an arrow.
    pub fn bolts(self, bolt: Option<opensacred_rules::arts::Bolt>) -> Self {
        Self {
            bolt,
            fires: bolt,
            ..self
        }
    }

    /// Its bolts of the clip's `bolt`, looses `fires` (`0x4DCD70`, `0x4DD92F`, #3030): with
    /// both effects the effect 16's clip and the effect 17's bolts, and the effect 16's bolt
    /// only while its staff is in the right hand (`0x4DE38C`), none else.
    pub fn firing(self, fires: Option<opensacred_rules::arts::Bolt>) -> Self {
        Self { fires, ..self }
    }

    /// Whether it is a shot, which a call to fight may reach in its course (`ai`, #3313).
    pub fn is_shot(&self) -> bool {
        self.shot.is_some()
    }

    /// Its target `target` from now, an arrow not loosed yet flying at it (`+0x158 := +0x104`,
    /// `0x4DC503`–`0x4DC577`, #3313).
    pub fn retarget(&mut self, target: Entity) {
        self.target = target;
    }
}

/// The name of `MOTIONS.PAK`'s record 0, whose fractions a slot that a model lacks takes, its
/// entry of the model's slot table 0 (`0x4135D0`; docs/02-data-formats.md §4.1).
const LACKING: &str = "INVALID_MOTION";

/// The clip that a shot of bolts plays (`0x4DCB80`'s sub-state 0 through `0x593320` and the
/// models' clip getter `0x4118B0`; docs/re/motions.md, "Where the model lacks the slot"; #3303).
enum BoltClip {
    /// The clip of its slot, at the fractions of its name, or, where the model lacks the slot, of
    /// its first stand-in that the model has (`minds::stand_ins`), at the fractions of [`LACKING`].
    Plays(AnimationNodeIndex, Handle<AnimationClip>, [f32; 2]),
    /// None, the slot lacking without a stand-in (the railgun's 164): the clip in play goes on.
    Kept,
}

/// The clip of a shot of `bolt` for an NPC's model ([`BoltClip`]).
fn bolt_clip(
    npc: &mut Npc,
    bolt: opensacred_rules::arts::Bolt,
    data: &Npcs,
    (gltfs, graphs): (&Assets<Gltf>, &mut Assets<AnimationGraph>),
) -> BoltClip {
    let fractions = |name: &str| {
        data.hits
            .get(&name.to_ascii_uppercase())
            .copied()
            .unwrap_or_default()
    };
    if let Some((node, clip)) = slot_clip(npc, bolt.slot(), data, (gltfs, &mut *graphs)) {
        let own = data
            .slots
            .get(&npc.model)
            .and_then(|slots| slots.slot(bolt.slot()))
            .map(&fractions)
            .unwrap_or_default();
        return BoltClip::Plays(node, clip, own);
    }
    for &slot in minds::stand_ins(bolt.slot()) {
        if let Some((node, clip)) = slot_clip(npc, slot, data, (gltfs, &mut *graphs)) {
            return BoltClip::Plays(node, clip, fractions(LACKING));
        }
    }
    BoltClip::Kept
}

/// Holds each medium spider in its shot (`creature_casts::ShotHeld`, #3697): at each world tick,
/// until less than 0.05 of its standing clip's loop is left, or the clip has ended, then two ticks
/// more (`0x4E55FD`–`0x4E574D`, `opensacred_rules::casts::shot_ends`), after which its cast ends.
fn hold_shots(
    mut commands: Commands,
    tick: Res<WorldTick>,
    mut held: Query<(Entity, &Npc, &mut crate::creature_casts::ShotHeld)>,
    players: Query<(&AnimationPlayer, &AnimationTransitions)>,
    (graphs, clips): (Res<Assets<AnimationGraph>>, Res<Assets<AnimationClip>>),
) {
    if tick.seconds.is_none() {
        return;
    }
    for (entity, npc, mut shot) in &mut held {
        match shot.0 {
            Some(0) => {
                commands
                    .entity(entity)
                    .remove::<crate::creature_casts::ShotHeld>();
            }
            Some(ticks) => shot.0 = Some(ticks - 1),
            None => {
                let share = npc
                    .player
                    .and_then(|player| players.get(player).ok())
                    .zip(npc.graph.as_ref())
                    .and_then(|((player, transitions), graph)| {
                        loop_share(transitions, player, graph, (&graphs, &clips))
                    });
                if opensacred_rules::casts::shot_ends(share.map(|share| 1.0 - share)) {
                    shot.0 = Some(opensacred_rules::casts::SHOT_TICKS_AFTER - 1);
                }
            }
        }
    }
}

/// The share of its loop that an NPC's clip in play has played, its time in its clip over the
/// clip's length, which a shot that plays no clip reads (`0x4DCB80`, #3562).
fn loop_share(
    transitions: &AnimationTransitions,
    player: &AnimationPlayer,
    graph: &Clips,
    (graphs, clips): (&Assets<AnimationGraph>, &Assets<AnimationClip>),
) -> Option<f32> {
    let main = transitions.get_main_animation()?;
    let bevy::animation::graph::AnimationNodeType::Clip(clip) =
        &graphs.get(&graph.graph)?.get(main)?.node_type
    else {
        return None;
    };
    let length = clips.get(clip)?.duration();
    let active = player.animation(main)?;
    (length > 0.0).then(|| active.seek_time() / length)
}

/// A creature whose idle handler searches a way to its target as it takes it up after its shot
/// (`0x4F7710` at `0x544217`, `opensacred_rules::minds::searches_after_shot`), which sets its
/// standing clip in play to its pace's rate ([`paced`], #3562).
#[derive(Component)]
pub struct SearchesWay;

/// Sets each searching creature's standing clip in play to its pace's rate, the game's speed
/// times its pace, keeping its place in its loop, where the blend that started it is over
/// (`0x4F75F0` through `0x413610`; `opensacred_rules::minds::search_paces`; docs/re/motions.md,
/// "The rate of the clip in play", #3562): until its next clip starts, as the ice druid's stance
/// loops in 0.5046 s after some of its shots. Before the blend's end the clip blending out takes
/// the rate, which shows nothing, and the standing clip plays on at 1.0; a clip that is no
/// standing one, which a decision started, keeps its own.
fn paced(
    mut commands: Commands,
    searching: Query<(Entity, &Npc), With<SearchesWay>>,
    mut players: Query<(&mut AnimationPlayer, &AnimationTransitions)>,
    speed: Res<hero::GameSpeed>,
) {
    for (entity, npc) in &searching {
        commands.entity(entity).remove::<SearchesWay>();
        let (Some(graph), Some(player)) = (&npc.graph, npc.player) else {
            continue;
        };
        let Ok((mut player, transitions)) = players.get_mut(player) else {
            continue;
        };
        let Some(main) = transitions
            .get_main_animation()
            .filter(|&main| graph.again(main).is_some())
        else {
            continue;
        };
        if let Some(active) = player.animation_mut(main)
            && minds::search_paces(active.elapsed())
        {
            bevy::log::debug!("npcs: {entity}'s search sets its standing clip to its pace's rate");
            active.set_speed(speed.0 * npc.pace);
        }
    }
}

/// An NPC whose strike, not a shot, has just played its clip whole: its mind decides whether the
/// next one chains (`ai::pursue`, `opensacred_rules::minds::Mind::ends_strike`, `0x4E4CF0`).
#[derive(Component)]
pub struct StrikeEnded;

/// An NPC whose shot has just ended, standing dormant until the hero's look wakes it and its
/// decisions take it up again (`ai::pursue`; the ticks since its wake, none before it).
#[derive(Component, Default)]
pub struct ShotEnded {
    pub woken: Option<u8>,
}

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
/// first fraction (0.5 for one of 0), or at its end, sends a hit of its share when the target is
/// then within reach; a creature of the heroes' class alone a second, where its clip's second
/// fraction is above 0 or with two weapons whatever the clip (#2748, #3686), as the action steps
/// at the ticks alone (#617); a walk ends the strike (`0x4E4CF0`, `0x4EA1F0`). A shot plays its clip at its attack speed times its chain, and at the first tick
/// past the clip's first fraction, or at its end, looses its arrow (`arrows::Loose`, `0x4DD90E`);
/// a shot of bolts plays its bolt's clip at its own rate, its stand-in where the model lacks it,
/// and sends its bolts in place of the arrow (`bolts::Fire`, `0x4DCB80`, #2899); one whose model
/// has neither plays nothing, the clip in play going on: the creature's stance, whose fraction is
/// 0, so that its first bolt leaves at the first tick after its start (#3303) and a railgun's
/// second at the first tick after it at which the stance has played 0.6 of its loop
/// (`opensacred_rules::arts::second_bolt_due`, #3562). A shot ends at the second tick after its
/// arrow or its last bolt, whatever is left of its clip (`0x4DCB80`'s sub-state 2; seen, #3312),
/// one that plays no clip at the first after a railgun's second bolt
/// (`opensacred_rules::arts::Bolt::ends_after`, #3572), its creature then dormant
/// ([`ShotEnded`]); one that played no clip starts its standing clip anew at the rate 1.0,
/// blended in over 0.2 s from its other node ([`Clips::again`]).
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
            &mut Npc,
            &mut Strikes,
            Has<Walk>,
            &GlobalTransform,
            Option<&fight::Combat>,
            (Option<&crate::ai::Mind>, Option<&Arms>),
        ),
        (
            Without<Dies>,
            Without<crate::making::Knocked>,
            Without<crate::backbreaker::Flung>,
        ),
    >,
    places: Query<(
        &GlobalTransform,
        Option<&hero::Size>,
        Option<&crate::ai::Mind>,
    )>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    (clips, gltfs, mut graphs, data): (
        Res<Assets<AnimationClip>>,
        Res<Assets<Gltf>>,
        ResMut<Assets<AnimationGraph>>,
        Res<Npcs>,
    ),
    (speed, factors): (Res<hero::GameSpeed>, Res<crate::motions::Factors>),
    mut hits: MessageWriter<fight::Hit>,
    (mut shots, mut bolts): (
        MessageWriter<arrows::Loose>,
        MessageWriter<crate::bolts::Fire>,
    ),
    (mut blows, mut fires): (
        MessageWriter<crate::sfx::Blow>,
        MessageWriter<crate::sfx::Gunfire>,
    ),
) {
    for (entity, mut npc, mut strikes, walking, at, combat, (mind, arms)) in &mut strikers {
        // A shot of bolts takes its bolt's clip, its stand-in or none ([`BoltClip`]).
        let bolt_clip = match (strikes.playing, strikes.bolt) {
            (None, Some(bolt)) => Some(bolt_clip(&mut npc, bolt, &data, (&gltfs, &mut graphs))),
            _ => None,
        };
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
            // No clip: its first bolt at the first tick after its start, a railgun's second at the
            // first tick after it at which the clip in play has played 0.6 of its loop, and the
            // shot's end at the first tick after a railgun's second bolt, the second after a BFG's,
            // which starts its standing clip anew at the rate 1.0 (`0x4DCB80`, #3562, #3572).
            if let (Some(BoltClip::Kept), Some(bolt)) = (&bolt_clip, strikes.bolt) {
                if !std::mem::replace(&mut strikes.begun, true) || tick.seconds.is_none() {
                    continue;
                }
                let count = strikes.fires.unwrap_or(bolt).count();
                let struck = u32::from(strikes.struck);
                let looses = struck == 0
                    || (struck < count
                        && loop_share(&transitions, &player, graph, (&graphs, &clips))
                            .is_some_and(opensacred_rules::arts::second_bolt_due));
                if looses {
                    bevy::log::debug!(
                        "npcs: {entity} shoots its bolt {struck} in the clip it plays"
                    );
                    strikes.struck += 1;
                    let toward = places
                        .get(strikes.target)
                        .map_or(at.translation(), |(target, ..)| target.translation());
                    if let Some(bolt) = strikes.fires {
                        bolts.write(crate::bolts::Fire {
                            shooter: entity,
                            toward,
                            bolt,
                            only: Some(struck),
                        });
                    }
                } else if struck >= count {
                    strikes.after += 1;
                    if strikes.after >= strikes.fires.unwrap_or(bolt).ends_after() {
                        commands
                            .entity(entity)
                            .remove::<Strikes>()
                            .insert(ShotEnded::default());
                        if let Some(again) = transitions
                            .get_main_animation()
                            .and_then(|main| graph.again(main))
                        {
                            transitions
                                .play(
                                    &mut player,
                                    again,
                                    Duration::from_secs_f32(minds::SHOT_END_BLEND),
                                )
                                .repeat()
                                .set_speed(speed.0);
                        }
                    }
                }
                continue;
            }
            let index = usize::try_from(strikes.choice).unwrap_or(0) % graph.attacks.len();
            let (node, clip, fractions) = match bolt_clip {
                Some(BoltClip::Plays(node, clip, fractions)) => (node, clip, fractions),
                _ => graph.attacks[index].clone(),
            };
            let length = clips.get(&clip).map_or(1.0, AnimationClip::duration);
            // At its attack speed (`0x4E4CF0`), a shot's times its chain (`0x4DCB80`).
            let attack = combat.map_or(1.0, |combat| combat.clip_speed());
            // A riding black rider's shot at twice the rate (`0x4DCDCC`, #2288).
            let doubled = strikes.shot.is_some()
                && mind.is_some_and(|mind| mind.rides.is_some() && mind.creature == BLACK_RIDER);
            let chain = strikes.shot.unwrap_or(1.0) * if doubled { 2.0 } else { 1.0 };
            // A shot of bolts at its bolt's own rate by its attack speed, its chain started anew
            // as the creatures' decision sends it to each (`0x4DCB80`, `0x53A159`; seen: the ice
            // druid's BFG's at 2.1, its attack speed 140, shot after shot).
            // The Dwarf's sword strike that begins its attack at its share (`0x4E5229`, #3353).
            let share = mind.map_or(1.0, |mind| {
                let class = arms.map_or(0, |arms| arms.armed(Some(&*data)).class);
                opensacred_rules::minds::strike_clip_share(mind.creature, class, strikes.opens)
            });
            let rate = strikes
                .bolt
                .map_or(attack * chain * share, |bolt| bolt.rate(attack));
            // Times its motion's own factor, the bows' and the crossbows' of some types
            // (`0x413610`, #3567).
            let factor = factors.of(&clip);
            if factor != 1.0 {
                bevy::log::debug!("npcs: {entity}'s strike takes its motion's own factor {factor}");
            }
            transitions
                .play(&mut player, node, BLEND)
                .set_speed(speed.0 * rate * factor);
            strikes.playing = Some((node, length.max(0.01), fractions));
            // A shot's clip that starts fires a gun held (`0x4DD1FE`, `0x4DF442`; #1963); one of
            // bolts fires none.
            if strikes.shot.is_some() && strikes.bolt.is_none() && !strikes.resumed {
                fires.write(crate::sfx::Gunfire { shooter: entity });
            }
            strikes.resumed = false;
            continue;
        };
        let Some(active) = player.animation(node) else {
            commands.entity(entity).remove::<Strikes>();
            continue;
        };
        let done = active.seek_time() / length;
        // At a tick, or as the clip ends between two, which the next tick's thought then sees; a
        // second hit for a creature of the heroes' class alone, with two weapons whatever the clip
        // (`0x4EA1F0`, `0x4E726C`, #2748, #3686); the first marked whether or not the target is
        // within reach (`0x4EA304`), so that one beyond it takes nothing that swing.
        let due = (tick.seconds.is_some() || active.is_finished())
            && match (strikes.shot, strikes.struck) {
                (Some(_), 0) => opensacred_rules::minds::looses(fractions[0], done),
                (Some(_), _) => false,
                (None, struck) => opensacred_rules::minds::hit_due(
                    fractions,
                    struck,
                    done,
                    npc.hits.class,
                    mind.is_some_and(|mind| mind.class == minds::HEROES_CLASS),
                ),
            };
        if due && strikes.shot.is_some() {
            strikes.struck += 1;
            if strikes.bolt.is_some() {
                // Its bolts toward its target's place (the FX events of `0x4DCB80`), none where it
                // looses none (#3030).
                let toward = places
                    .get(strikes.target)
                    .map_or(at.translation(), |(target, ..)| target.translation());
                if let Some(bolt) = strikes.fires {
                    bolts.write(crate::bolts::Fire {
                        shooter: entity,
                        toward,
                        bolt,
                        only: None,
                    });
                }
            } else {
                blows.write(crate::sfx::Blow::Shot { shooter: entity });
                shots.write(arrows::Loose {
                    shooter: entity,
                    target: strikes.target,
                    at: None,
                    special: None,
                });
            }
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
                blows.write(crate::sfx::Blow::Swing {
                    striker: entity,
                    air: None,
                });
                hits.write(fight::Hit {
                    fixed: None,
                    sure: false,
                    poison: false,
                    ward: false,
                    hurled: false,
                    object: false,
                    mount: false,
                    marks: false,
                    attacker: entity,
                    target: strikes.target,
                    share: npc.hits.share,
                    ranged: false,
                    art: None,
                });
            }
        }
        // A shot's ticks after its arrow, the tick that loosed it aside.
        let shot = strikes.shot.is_some();
        if shot && strikes.struck > 0 && !due && tick.seconds.is_some() {
            strikes.after += 1;
        }
        if active.is_finished() || (shot && strikes.after >= 2) {
            let mut ended = commands.entity(entity);
            ended.remove::<Strikes>();
            if shot {
                ended.insert(ShotEnded::default());
            } else {
                ended.insert(StrikeEnded);
            }
        }
    }
}

/// Levels each boss, champion or creature of a champion's levelling still unfought
/// (`fight::Unfought`) at its first strike or hit, as the hit is sent and before it is resolved
/// (`0x56B130` for both sides, `0x550E50`, #630): its opponent's level, a creature's or the
/// hero's, plus `LevelUpBoss` or `LevelUpChamp`, held within the levels of the sector it stands
/// in, then through the difficulty's scaling with the survival counter and the hero's sets' part
/// where its opponent is the hero (`0x5510C5`, #1761; `opensacred_rules::fight::Rank::level`);
/// its health refilled to its new most, its items
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
    (experience, played): (Res<fight::Experience>, Option<Res<drops::Survival>>),
    heroes: Query<Option<&fight::Holding>, With<hero::Body>>,
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
            // The survival counter counts against a player alone (`0x5510D1`).
            let opponent = if let Ok(holding) = heroes.get(opponent) {
                let time = played
                    .as_deref()
                    .map_or_else(fight::Played::default, |played| played.time(holding));
                (experience.level, time)
            } else if let Ok(health) = levels.get(opponent) {
                (health.level, fight::Played::default())
            } else if let Ok((.., health, _, _, _, _)) = unfought.get(opponent) {
                (health.level, fight::Played::default())
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
                *values = npcs
                    .items
                    .make(held.item, grade, (rules.difficulty(), 0), &mut dice);
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
                 its opponent's {} and {} % of the survival counter, health {}",
                rank.0,
                opponent.0,
                opponent.1.per_cent(),
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
/// at each such kill, and bursts it into its sparks (`0x57F504`, `level_sparks`, #2762). The
/// hero's mount takes what the hero takes ([`mount_earns`], #3393); not followed for a creature's
/// mount or a horse's rider (`+0x1EC`); a follower of the hero earns nothing, its kill going to
/// the hero (`fight::resolve`, #3394); a blue potion's effect adds to the kill's experience
/// (#1353).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
pub(crate) fn earn(
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
            // A creature's count vector is the dummy: no set's share (`0x57DDE3`).
            (bonus, 0.0),
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
        commands
            .entity(kill.killer)
            .insert((regenerates, crate::level_sparks::Rises));
        bevy::log::info!(
            "npcs: {}, a {} of level {from}, reaches the level {reached} by its kills' \
             experience, held at {grade}, health {}",
            kill.killer,
            mind.creature,
            health.most
        );
    }
}

/// Gives the horse that the hero rides, where it rides one, the experience `amount` that the hero
/// took of a kill, whole, by the same `addExperience` (`0x5319B1`–`0x531A37`; docs/re/fight.md,
/// "The mount's and the rider's share", #3393): added to its points, and once they pass its
/// level's end the level of its points, held at 250 alone and not by its sector (`0x564D60` at
/// `0x5654EB`, `opensacred_rules::fight::held_level`), its values made anew and its health
/// refilled, with the level's sparks.
pub(crate) fn mount_earns(world: &mut bevy::prelude::World, amount: u32) {
    let horse = world
        .query_filtered::<&crate::riding::Seat, bevy::prelude::With<hero::Body>>()
        .iter(world)
        .next()
        .and_then(crate::riding::Seat::ridden);
    let Some(horse) = horse.filter(|_| world.contains_resource::<Npcs>()) else {
        return;
    };
    world.resource_scope(|world, npcs: bevy::prelude::Mut<Npcs>| {
        let rules = &npcs.fight;
        let mut horses = world.query::<(
            Option<&mut fight::Points>,
            (&mut ai::Mind, &mut fight::Health, &mut fight::Combat),
            (&Arms, &mut Npc),
            Option<&crate::effects::Effects>,
        )>();
        let Ok((points, (mut mind, mut health, mut combat), (arms, mut npc), effects)) =
            horses.get_mut(world, horse)
        else {
            return;
        };
        let from = health.level;
        let mut sum = points
            .as_deref()
            .map_or_else(|| fight::level_start(from), |points| points.0);
        let reached = fight::creature_earns(&mut sum, from, u64::from(amount));
        let had = points.is_some();
        if let Some(mut points) = points {
            points.0 = sum;
        }
        let creature = mind.creature;
        let raised = reached.map(|reached| {
            let grade =
                opensacred_rules::fight::held_level(reached, None, mind.creature, true, rules);
            let regenerates = relevel(
                grade,
                fight::Rank::Common,
                (&mut *mind, &mut *health, &mut *combat, &mut *npc),
                arms,
                rules,
                &effects
                    .map(crate::effects::Effects::list)
                    .unwrap_or_default(),
            );
            (grade, regenerates, health.most)
        });
        let mut entity = world.entity_mut(horse);
        if !had {
            entity.insert(fight::Points(sum));
        }
        bevy::log::info!(
            "npcs: the hero's horse {horse}, a {creature} of level {from}, earns {amount} \
             experience with its rider, {sum} in all"
        );
        if let Some((grade, regenerates, most)) = raised {
            entity.insert((regenerates, crate::level_sparks::Rises));
            bevy::log::info!(
                "npcs: the hero's horse {horse} reaches the level {grade}, health {most}"
            );
        }
    });
}

/// Sets the level of the follower `entity` as the hero of level `hero` sets it (#2896): at its
/// hire `always` (`sdk_HIRE`, `0x54D14C`), else only where it stands below the hero's level
/// scaled, as the hero's own setting of its level raises the followers it keeps (`0x564D60` at
/// its end): the level of [`opensacred_rules::fight::follower_level`], held by its own sector,
/// its values made anew at it as the level's setter makes them, its items as they are.
pub(crate) fn level_follower(
    world: &mut bevy::prelude::World,
    entity: Entity,
    hero: u32,
    always: bool,
) {
    world.resource_scope(|world, npcs: bevy::prelude::Mut<Npcs>| {
        let rules = &npcs.fight;
        let mut creatures = world.query::<(
            &Transform,
            (&mut ai::Mind, &mut fight::Health, &mut fight::Combat),
            (&Arms, &mut Npc),
            (Option<&fight::Ranked>, Option<&crate::effects::Effects>),
        )>();
        let Ok((transform, (mut mind, mut health, mut combat), (arms, mut npc), (ranked, effects))) =
            creatures.get_mut(world, entity)
        else {
            return;
        };
        if !always && health.level >= opensacred_rules::fight::follower_scale(hero, rules) {
            return;
        }
        let rank = ranked.map_or(fight::Rank::Common, |ranked| ranked.0);
        let values = npcs.values.get(&sector_of(transform.translation));
        let grade = opensacred_rules::fight::follower_level(
            hero,
            (rank, mind.creature, mind.big),
            values,
            rules,
        );
        let from = health.level;
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
        let creature = mind.creature;
        world.entity_mut(entity).insert(regenerates);
        bevy::log::info!(
            "npcs: the follower {entity}, a {creature} of level {from}, set to {grade} by the \
             hero's {hero}"
        );
    });
}

/// Sets the level of the creature `entity` that the Vampiress's awakening raises to `grade`
/// exactly (`0x564D60(…, 1, 1)` at `0x511088`; docs/re/arts.md, "The Vampiress's", #3140): a
/// boss common, the hire's mask taking its bit (`+0x1F4` `0x80000`), a champion still one (its
/// `+0x2B8` bit `0x40` untouched); its values made anew at it as the level's setter makes them,
/// its health filled, its items as they are.
pub(crate) fn raised_level(world: &mut bevy::prelude::World, entity: Entity, grade: u32) {
    world.resource_scope(|world, npcs: bevy::prelude::Mut<Npcs>| {
        let rules = &npcs.fight;
        let mut creatures = world.query::<(
            (&mut ai::Mind, &mut fight::Health, &mut fight::Combat),
            (&Arms, &mut Npc),
            (Option<&mut fight::Ranked>, Option<&crate::effects::Effects>),
        )>();
        let Ok(((mut mind, mut health, mut combat), (arms, mut npc), (ranked, effects))) =
            creatures.get_mut(world, entity)
        else {
            return;
        };
        let mut rank = ranked
            .as_deref()
            .map_or(fight::Rank::Common, |ranked| ranked.0);
        if rank == fight::Rank::Boss {
            rank = fight::Rank::Common;
            if let Some(mut ranked) = ranked {
                ranked.0 = rank;
            }
        }
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
        world.entity_mut(entity).insert(regenerates);
    });
}

/// Gives the creature `entity`, in the world, the level of a `SetNPCState`'s or a
/// `SetGroupState`'s `0x1F`, `tag`, as the original's records give it to one in the world
/// (`0x461540` at `0x46213D`; docs/re/spawning.md, "`SetNPCState`'s level"; #2894, #2966): 1000
/// makes it a boss of the marker level 1, levelled by the hero at once where it is hostile to
/// the hero, as one made so is (`fight::Rank::level`, docs/decisions/fight.md, "Bosses and
/// champions levelled when made"), else at its first fight (`fight::Unfought`); any other level
/// as the level's setter takes it, scaled and held
/// (`opensacred_rules::fight::tagged_level`); its values made anew at it, its items as they are.
pub(crate) fn state_level(world: &mut bevy::prelude::World, entity: Entity, tag: u32) {
    if tag == 0 {
        return;
    }
    let hero = world
        .get_resource::<fight::Experience>()
        .map_or(fight::HERO_LEVEL, |experience| experience.level);
    let mut heroes = world.query_filtered::<&fight::Holding, With<hero::Body>>();
    let time = {
        let world = &*world;
        let holding = heroes.iter(world).next();
        world
            .get_resource::<drops::Survival>()
            .map_or_else(fight::Played::default, |played| played.time(holding))
    };
    world.resource_scope(|world, npcs: bevy::prelude::Mut<Npcs>| {
        let rules = &npcs.fight;
        let mut creatures = world.query::<(
            &Transform,
            (&mut ai::Mind, &mut fight::Health, &mut fight::Combat),
            (&Arms, &mut Npc),
            (&mut fight::Ranked, Option<&crate::effects::Effects>),
        )>();
        let Ok((
            transform,
            (mut mind, mut health, mut combat),
            (arms, mut npc),
            (mut ranked, effects),
        )) = creatures.get_mut(world, entity)
        else {
            return;
        };
        let values = npcs
            .values
            .get(&sector_of(transform.translation))
            .copied()
            .unwrap_or_default();
        let boss = tag == fight::BOSS_TAG;
        let waits = boss && !minds::hostile_to_heroes(mind.side, mind.creature);
        let (rank, grade) = if boss {
            let rank = fight::Rank::Boss;
            let grade = if waits {
                1
            } else {
                rank.level(1, (hero, time), &values, mind.big, rules)
            };
            (rank, grade)
        } else {
            let horse = mind.class == HORSES_CLASS;
            let grade = opensacred_rules::fight::tagged_level(
                tag,
                Some(&values),
                mind.creature,
                horse,
                rules,
            )
            .unwrap_or(health.level);
            (ranked.0, grade)
        };
        ranked.0 = rank;
        let from = health.level;
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
        let (creature, big) = (mind.creature, mind.big);
        let mut made = world.entity_mut(entity);
        made.insert(regenerates);
        if waits {
            made.insert(fight::Unfought { big });
        }
        bevy::log::info!(
            "npcs: the scripts set {entity}, a {creature} of level {from}, to {grade} as a \
             {rank:?} by its level {tag}"
        );
    });
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
    let factor = rank.health(mind.creature, mind.class, rules);
    let most = arms.most_health(grade, factor, rules);
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
    mind.fly *= ratio;
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
    let (values, hits) = arms.fight_with(grade, rank, rules, effects);
    // Its pace times its time's factor, the effect 15's (`0x537160`).
    let pace = values.0.pace_factor() * opensacred_rules::fight::time_factor(effects);
    repace(mind, npc, pace);
    npc.hits = hits;
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
    /// The items it wears, each its type, its category and its values, made at its level
    /// (#2286).
    worn: Vec<(u32, u8, Option<fight::ItemValues>)>,
    /// Whether it rides a horse, a fill's rider seated as it is made (`npcs::seat_rider`), whose
    /// riding skill weighs its `A` multiplier (`0x552660`, #2385).
    mounted: bool,
}

impl Arms {
    /// The class of what it holds (`motions::class`, `0x546F80`).
    pub fn class(&self) -> u8 {
        let in_hands: Vec<InHand> = self.holding.iter().map(|&(held, _)| held).collect();
        class_of(&in_hands)
    }

    /// Whether it holds a ranged weapon in a hand ([`ranged_of`], `0x54A2A0`).
    pub fn ranged(&self) -> bool {
        let in_hands: Vec<InHand> = self.holding.iter().map(|&(held, _)| held).collect();
        ranged_of(&in_hands)
    }

    /// What it holds, for its weapons' sounds ([`Npcs::armed`]); bare hands without the NPCs'
    /// tables.
    pub fn armed(&self, npcs: Option<&Npcs>) -> crate::sfx::Armed {
        let in_hands: Vec<InHand> = self.holding.iter().map(|&(held, _)| held).collect();
        npcs.map_or_else(crate::sfx::Armed::default, |npcs| npcs.armed(&in_hands))
    }

    /// The type of the item in its right hand (slot 13) and its values, if it holds one.
    pub fn right(&self) -> Option<(u32, Option<fight::ItemValues>)> {
        self.holding
            .iter()
            .find(|(held, _)| held.hand == Hand::Right)
            .map(|&(held, values)| (held.item, values))
    }

    /// The type of the item in its left hand (slot 12), if any.
    pub fn left(&self) -> Option<u32> {
        self.holding
            .iter()
            .find(|(held, _)| held.hand == Hand::Left)
            .map(|(held, _)| held.item)
    }

    /// Its attack, defence, damage and resistances at level `grade` and of rank `rank`
    /// (`0x5796A0`), with its skills and its items' values: unarmed when what it holds is of
    /// class 0 and its right hand empty (`0x546F80`); and its hits' share of a strike, halved with
    /// two weapons, the class 8 or one in each hand, the left no shield (`0x4EA3F0`), with the
    /// class by which its strikes hit ([`Hits`]).
    fn fight(&self, grade: u32, rank: fight::Rank, rules: &fight::Rules) -> (fight::Combat, Hits) {
        self.fight_with(grade, rank, rules, &[])
    }

    /// The values of the items in its hands, of those that have some.
    pub(crate) fn held_values(&self) -> Vec<fight::ItemValues> {
        self.holding
            .iter()
            .filter_map(|&(_, values)| values)
            .collect()
    }

    /// The type of the first item it wears of the category `category` (a horse's bridle in its
    /// slot 1, #2907).
    pub(crate) fn wears(&self, category: u8) -> Option<u32> {
        self.worn
            .iter()
            .find(|&&(_, worn, _)| worn == category)
            .map(|&(item, ..)| item)
    }

    /// The values of every item it holds and wears, of those that have some, which the getter of
    /// the items' effects sums (`0x570ED0`, #2286).
    pub(crate) fn item_values(&self) -> Vec<fight::ItemValues> {
        self.held_values()
            .into_iter()
            .chain(self.worn.iter().filter_map(|&(_, _, values)| values))
            .collect()
    }

    /// The item in the hand of the equipment's slot `slot` (12 the left, 13 the right,
    /// `0x556F50`) and its values.
    pub(crate) fn in_hand(&self, slot: u8) -> Option<(u32, Option<fight::ItemValues>)> {
        let hand = Hand::of_slot(slot)?;
        self.holding
            .iter()
            .find(|(held, _)| held.hand == hand)
            .map(|&(held, values)| (held.item, values))
    }

    /// Its level in the skill `skill` at the level `grade` (`0x582450`), with its items' bonus, 0
    /// without it ([`Self::skills`]).
    pub(crate) fn skill_level(&self, grade: u32, skill: u8) -> u32 {
        self.skills(grade)
            .iter()
            .find(|&&(had, _)| had == skill)
            .map_or(0, |&(_, level)| level)
    }

    /// Whether it has the skill of two weapons (6) at the level `grade`, its level in the skill
    /// above 0 with its items' bonus (`0x582450(6)`).
    pub(crate) fn two_weapons(&self, grade: u32) -> bool {
        self.skills(grade)
            .iter()
            .any(|&(skill, level)| skill == 6 && level > 0)
    }

    /// Its skills and their levels at the level `grade`, as its fight counts them
    /// (`fight::skills`: its type's slots at the level's setter's levels, `0x564D60`), each with
    /// its items' bonus, at most 255 (`opensacred_rules::fight::skills_held`, `0x582450`, #2948).
    pub(crate) fn skills(&self, grade: u32) -> Vec<(u8, u32)> {
        opensacred_rules::fight::skills_held(&self.base, grade, self.plain, &self.holds())
    }

    /// [`Self::fight`] with its effects `effects` (#763).
    fn fight_with(
        &self,
        grade: u32,
        rank: fight::Rank,
        rules: &fight::Rules,
        effects: &[Effect],
    ) -> (fight::Combat, Hits) {
        let held = self.holds();
        let hits = Hits {
            class: held.class,
            share: held.share(),
        };
        let skills = opensacred_rules::fight::skills_held(&self.base, grade, self.plain, &held);
        let mut values =
            opensacred_rules::fight::combat_with(&self.base, grade, held, &skills, effects, rules);
        let [attack, defence] = rank.fight(rules);
        values.attack *= attack;
        values.defence *= defence;
        (fight::Combat(values), hits)
    }

    /// Its most health at level `grade` and of the rank's factor `rank` (`0x5658F0`): by its
    /// base, its level, its skill 10 and its items' bonuses to the attributes `a0` and `a3`
    /// (#1365); a crab's or a fish's the stats' own.
    fn most_health(&self, grade: u32, rank: f32, rules: &fight::Rules) -> u32 {
        if self.plain {
            return PLAIN_HEALTH;
        }
        let held = self.holds();
        let skills = opensacred_rules::fight::skills_held(&self.base, grade, self.plain, &held);
        fight::most_health_held(&self.base, grade, &held, &skills, rank, rules)
    }

    /// What it holds as its values take it: unarmed when what it holds is of class 0 and its
    /// right hand empty (`0x546F80`), its items' values, the class of what it holds and whether it
    /// strikes with two weapons ([`two_weapons_of`]).
    fn holds(&self) -> fight::Holds {
        let in_hands: Vec<InHand> = self.holding.iter().map(|&(held, _)| held).collect();
        let class = class_of(&in_hands);
        let holds = |hand, shield: Option<bool>| {
            in_hands.iter().any(|held| {
                held.hand == hand && shield.is_none_or(|s| (held.category == SHIELD) == s)
            })
        };
        let right = holds(Hand::Right, None);
        fight::Holds {
            weapon: class != 0 || right,
            shield: holds(Hand::Left, Some(true)),
            ranged: ranged_of(&in_hands),
            gun: holds_kind(&in_hands, &GUNS),
            // The items in its hands and those it wears (#2286).
            items: self
                .holding
                .iter()
                .filter_map(|&(held, values)| Some((held.category, values?)))
                .chain(
                    self.worn
                        .iter()
                        .filter_map(|&(_, category, values)| Some((category, values?))),
                )
                .map(|(class, values)| fight::ItemValues { class, ..values })
                .collect(),
            skill: skill_of(class, &self.holding),
            // A creature not a player's takes no set's bonus (`0x57AE56`).
            sets: Default::default(),
            mounted: self.mounted,
            class,
            two: two_weapons_of(&in_hands, class),
            // Worked out of their values: a creature's items have no sockets filled.
            levels: Vec::new(),
        }
    }
}

/// An NPC that changes its weapons (#396; docs/re/fight.md, "The archers"): it puts away what
/// its hands `away` hold, and takes the items `takes`, each in its hand, with its values where
/// they are given, else made at its level.
#[derive(Component, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Rearms {
    away: Vec<Hand>,
    takes: Vec<(u32, Hand, Option<fight::ItemValues>)>,
    /// Whether it is disarmed (`0x4E9900`, #2298): its values made anew keep a share alone
    /// (`opensacred_rules::fight::disarmed`).
    disarmed: bool,
    /// The items it puts on that go in no hand, each its type, its category, its model where it
    /// has one (an `EquipNPC`'s, #2756) and its values where they are given, else made at its
    /// level.
    wears: Vec<(u32, u8, Option<Held>, Option<fight::ItemValues>)>,
}

impl MapEntities for Rearms {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, _: &mut E) {}
}

impl crate::doings::Saves for Rearms {
    type Saved = Rearms;

    fn saved(&self, _: &crate::doings::Moment) -> Option<Self> {
        Some(self.clone())
    }

    fn again(saved: Self, _: &crate::doings::Moment) -> Option<Self> {
        Some(saved)
    }
}

impl Rearms {
    /// An archer's switch to melee (`0x4D4B70`): the bow of its left hand away (slot 12), and
    /// `weapon`, if any, in its right (slot 13).
    pub fn to_melee(weapon: Option<u32>) -> Self {
        Self {
            away: match weapon {
                Some(_) => vec![Hand::Left, Hand::Right],
                None => vec![Hand::Left],
            },
            takes: weapon
                .map(|weapon| (weapon, Hand::Right, None))
                .into_iter()
                .collect(),
            disarmed: false,
            wears: Vec::new(),
        }
    }

    /// `item` put into its right hand (slot 13) in place of what it held there, made by the item
    /// maker at its level: the goblin god's child's wand (`0x555E00`, #3225).
    pub fn right(item: u32) -> Self {
        Self {
            away: vec![Hand::Right],
            takes: vec![(item, Hand::Right, None)],
            disarmed: false,
            wears: Vec::new(),
        }
    }

    /// Its right hand emptied (slot 13): the staff of the Seraphim's effect 16 taken out as the
    /// effect goes (`0x586D80`, `bolt_hands`, #3030).
    pub fn empty_right() -> Self {
        Self {
            away: vec![Hand::Right],
            takes: Vec::new(),
            disarmed: false,
            wears: Vec::new(),
        }
    }

    /// Both its hands emptied (slots 12 and 13): the Seraphim's effect 17 come (`0x582D50`,
    /// `bolt_hands`, #3030).
    pub fn empty_hands() -> Self {
        Self {
            away: vec![Hand::Left, Hand::Right],
            takes: Vec::new(),
            disarmed: false,
            wears: Vec::new(),
        }
    }

    /// The bow-taking (`0x4D4A80`): its right hand emptied (slot 13), and `bow` in its left
    /// (slot 12).
    pub fn bow(bow: u32) -> Self {
        Self {
            away: vec![Hand::Right, Hand::Left],
            takes: vec![(bow, Hand::Left, None)],
            disarmed: false,
            wears: Vec::new(),
        }
    }

    /// A guard's or a townsperson's torch by night and by day (`minds::Torch`, #61): a torch
    /// taken into its left hand (slot 12), or what that hand holds put away.
    pub fn torch(torch: opensacred_rules::minds::Torch) -> Self {
        Self {
            away: vec![Hand::Left],
            takes: match torch {
                opensacred_rules::minds::Torch::Takes => {
                    vec![(opensacred_rules::minds::TORCH, Hand::Left, None)]
                }
                opensacred_rules::minds::Torch::PutsAway => Vec::new(),
            },
            disarmed: false,
            wears: Vec::new(),
        }
    }

    /// A minimummy's dagger taken into its empty right hand (slot 13, `0x4D93C1`, #2575), its
    /// values made at its level.
    pub fn dagger() -> Self {
        Self {
            away: Vec::new(),
            takes: vec![(opensacred_rules::casts::DAGGER, Hand::Right, None)],
            disarmed: false,
            wears: Vec::new(),
        }
    }

    /// A new item of the type `item` and the category `category` put on, made at its level, of no
    /// model (a restocked horse's bridle, `0x538199`, #2907).
    pub(crate) fn wearing(item: u32, category: u8) -> Self {
        Self {
            away: Vec::new(),
            takes: Vec::new(),
            disarmed: false,
            wears: vec![(item, category, None, None)],
        }
    }

    /// A script's `EquipNPC` on a creature in the world (`0x481D80`, #2756), what it holds and
    /// wears after it `arming`: both hands emptied and its items in their hands taken again, those
    /// it held before with their values, and its items worn that it did not wear put on.
    fn equipping(arming: &Arming, worn_before: &[u32]) -> Self {
        Self {
            away: vec![Hand::Left, Hand::Right],
            takes: arming
                .holding
                .iter()
                .map(|held| (held.item, held.hand, None))
                .collect(),
            disarmed: false,
            wears: arming
                .worn
                .iter()
                .filter(|(item, _)| !worn_before.contains(item))
                .map(|&(item, category)| {
                    let model = arming
                        .equipment
                        .iter()
                        .find(|held| held.item == item && held.mount == Mount::Worn)
                        .cloned();
                    (item, category, model, None)
                })
                .collect(),
        }
    }

    /// The item of its right hand thrown by the hurled blades (#2575): its hand emptied.
    pub fn throws() -> Self {
        Self {
            away: vec![Hand::Right],
            takes: Vec::new(),
            disarmed: false,
            wears: Vec::new(),
        }
    }

    /// A disarming (`0x4E9900`, #2298): what `hand` holds thrown away, else nothing more where
    /// it took a worn item off ([`disarm`]), its values made anew of what it holds then, of which
    /// it keeps a share alone.
    fn disarm(hand: Option<Hand>) -> Self {
        Self {
            away: hand.into_iter().collect(),
            takes: Vec::new(),
            disarmed: true,
            wears: Vec::new(),
        }
    }

    /// A hireling's hands changed through its portrait (`0x6D6760`, #1771): the items of the
    /// hands' slots `off` (12 the left, 13 the right) taken off, and an item with its values put
    /// into the slot of `takes`, which empties first.
    pub(crate) fn handing(off: &[u8], takes: Option<(u32, u8, fight::ItemValues)>) -> Self {
        let away = off
            .iter()
            .copied()
            .chain(takes.map(|(_, slot, _)| slot))
            .filter_map(Hand::of_slot)
            .collect();
        Self {
            away,
            takes: takes
                .and_then(|(item, slot, values)| Some((item, Hand::of_slot(slot)?, Some(values))))
                .into_iter()
                .collect(),
            disarmed: false,
            wears: Vec::new(),
        }
    }
}

/// Disarms `entity` (`0x4E9900`, read 2026-11-08 for #2298): an NPC that is no follower of the
/// hero's and that the scripts keep to rise again (`+0x2B7` bit 8, [`RisesAgain`], #3088) throws
/// down what its right hand holds, else its left (the slots 13 and 12), from its
/// model's place, the item with its values ([`crate::drops::throw_disarmed`]); a living one's
/// values are made anew of what it holds then, of which it keeps a share
/// (`opensacred_rules::fight::disarmed`), a dying one's model alone goes. With its hands empty it
/// takes off what it wears of the slot 1, else the slot 0 (the armour, the helmet; #2816), from
/// its neck's bone ([`disarm_worn`]).
pub(crate) fn disarm(world: &mut bevy::prelude::World, entity: Entity) {
    if world.get::<crate::followers::Follower>(entity).is_some()
        || world.get::<RisesAgain>(entity).is_some()
    {
        return;
    }
    let Some(arms) = world.get::<Arms>(entity) else {
        return;
    };
    let held = |hand: Hand| {
        arms.holding
            .iter()
            .find(|(held, _)| held.hand == hand)
            .map(|&(held, values)| (held, values))
    };
    let Some((held, values)) = held(Hand::Right).or_else(|| held(Hand::Left)) else {
        disarm_worn(world, entity);
        return;
    };
    throw_held(world, entity, (held, values));
}

/// Throws down what `entity`'s hand of the equipment's slot `slot` holds, as its disarming does
/// ([`disarm`]): a skeleton warrior's item of the arm it loses as it changes its type (`0x550580`
/// calling `0x4E9900` with the slot, `undead::change`, #3402); nothing where that hand holds
/// none.
pub(crate) fn throw_hand(world: &mut bevy::prelude::World, entity: Entity, slot: u8) {
    let Some(hand) = Hand::of_slot(slot) else {
        return;
    };
    let held = world.get::<Arms>(entity).and_then(|arms| {
        arms.holding
            .iter()
            .find(|(held, _)| held.hand == hand)
            .map(|&(held, values)| (held, values))
    });
    if let Some(held) = held {
        throw_held(world, entity, held);
    }
}

/// What a creature holds in its hands and wears, each item with its values, those it is to take
/// or put on still among them, and each worn one's model, which an undead keeps as it rises or
/// changes its type, its object the same in the original (`0x555C50` puts its items back, #3402,
/// #3477), and which the creature made in its place holds and wears in place of its type's
/// ([`KeptItems::give`], `undead::remake`).
#[derive(Clone, Debug, Default)]
pub(crate) struct KeptItems {
    held: Vec<(u32, Hand, Option<fight::ItemValues>)>,
    worn: Vec<(u32, u8, Option<Held>, Option<fight::ItemValues>)>,
}

impl KeptItems {
    /// What `entity` holds and wears and is to take and put on.
    pub(crate) fn of(world: &mut bevy::prelude::World, entity: Entity) -> Self {
        let mut pieces = world.query::<(&WornBy, &Piece)>();
        let mut models: Vec<Piece> = pieces
            .iter(world)
            .filter(|(by, piece)| by.0 == entity && !piece.mount.handed())
            .map(|(_, piece)| piece.clone())
            .collect();
        if let Some(equipment) = world.get::<Equipment>(entity) {
            models.extend(
                equipment
                    .0
                    .iter()
                    .filter(|equip| !equip.piece.mount.handed())
                    .map(|equip| equip.piece.clone()),
            );
        }
        let (mut held, mut worn) = world.get::<Arms>(entity).map_or_else(
            || (Vec::new(), Vec::new()),
            |arms| {
                let held: Vec<_> = arms
                    .holding
                    .iter()
                    .map(|(held, values)| (held.item, held.hand, *values))
                    .collect();
                let worn: Vec<_> = arms
                    .worn
                    .iter()
                    .map(|&(item, category, values)| {
                        let model = models
                            .iter()
                            .find(|piece| piece.item == item)
                            .map(Held::of_piece);
                        (item, category, model, values)
                    })
                    .collect();
                (held, worn)
            },
        );
        if let Some(rearms) = world.get::<Rearms>(entity) {
            held.retain(|(_, hand, _)| !rearms.away.contains(hand));
            held.extend(rearms.takes.iter().cloned());
            worn.extend(rearms.wears.iter().cloned());
        }
        Self { held, worn }
    }

    /// Has `entity` hold and wear these items alone ([`Rearms`]), once it has a mind: what it held
    /// and wore before goes ([`strip`]).
    pub(crate) fn give(&self, world: &mut bevy::prelude::World, entity: Entity) {
        strip(world, entity);
        if let Ok(mut taker) = world.get_entity_mut(entity) {
            taker.insert(Rearms {
                away: vec![Hand::Left, Hand::Right],
                takes: self.held.clone(),
                disarmed: false,
                wears: self.worn.clone(),
            });
        }
    }
}

/// Throws down the item `held` with its values from `entity`'s hand ([`disarm`]).
fn throw_held(
    world: &mut bevy::prelude::World,
    entity: Entity,
    (held, values): (InHand, Option<fight::ItemValues>),
) {
    let level = world.get::<ai::Mind>(entity).map_or(1, |mind| mind.level);
    let floor = world
        .get::<crowd::InTheWay>(entity)
        .and_then(|way| way.level);
    let mut models = world.query::<(Entity, &Wielded, &models::OrderedWith, &GlobalTransform)>();
    let model = models
        .iter(world)
        .find(|(_, wielded, by, _)| by.0 == entity && wielded.0 == held.hand)
        .map(|(model, _, _, at)| (model, at.translation()));
    let at = model.map(|(_, at)| at).or_else(|| {
        world
            .get::<GlobalTransform>(entity)
            .map(GlobalTransform::translation)
    });
    let Some(at) = at else {
        return;
    };
    bevy::log::debug!("npcs: {entity} is disarmed of {}", held.item);
    if world.get::<Dies>(entity).is_some() {
        if let Some((model, _)) = model {
            world.despawn(model);
        }
        if let Some(mut arms) = world.get_mut::<Arms>(entity) {
            arms.holding.retain(|(other, _)| other.hand != held.hand);
        }
    } else {
        world
            .entity_mut(entity)
            .insert(Rearms::disarm(Some(held.hand)));
    }
    if let Err(error) = world.run_system_cached_with(
        crate::drops::throw_disarmed,
        ((held.item, level, values), (at, floor)),
    ) {
        bevy::log::warn!("npcs: {entity}'s weapon is not thrown down: {error}");
    }
}

/// Destroys every item that `entity` holds and wears, as the changeling's effect 20 does as it
/// changes the creature (`0x5848A8`, #3146): their models gone, out of its [`Arms`] and its
/// [`Equipment`], nothing thrown down; its values made anew without them once it has a mind again
/// ([`Rearms`] of both hands, which waits for it).
pub(crate) fn strip(world: &mut bevy::prelude::World, entity: Entity) {
    let mut wielded = world.query::<(Entity, &Wielded, &models::OrderedWith)>();
    let mut pieces = world.query::<(Entity, &WornBy, &Piece)>();
    let models: Vec<Entity> = wielded
        .iter(world)
        .filter(|(_, _, by)| by.0 == entity)
        .map(|(model, ..)| model)
        .chain(
            pieces
                .iter(world)
                .filter(|(_, by, _)| by.0 == entity)
                .map(|(model, ..)| model),
        )
        .collect();
    for model in models {
        world.despawn(model);
    }
    if let Some(mut arms) = world.get_mut::<Arms>(entity) {
        bevy::log::debug!(
            "npcs: {entity}'s items go: {:?} held, {:?} worn",
            arms.holding
                .iter()
                .map(|(held, _)| held.item)
                .collect::<Vec<_>>(),
            arms.worn.iter().map(|&(item, ..)| item).collect::<Vec<_>>()
        );
        arms.holding.clear();
        arms.worn.clear();
    }
    if let Some(mut equipment) = world.get_mut::<Equipment>(entity) {
        equipment.0.clear();
    }
    world.entity_mut(entity).insert(Rearms {
        away: vec![Hand::Left, Hand::Right],
        takes: Vec::new(),
        disarmed: false,
        wears: Vec::new(),
    });
}

/// The equipment's slots whose worn item the disarming takes off where the hands are empty, in
/// turn: the armour's and the helmet's (`0x4E9900`).
const DISARMED_WORN: [u8; 2] = [1, 0];

/// The bone from which the disarming throws an item that has no socket (`0x4E9900`).
const DISARMED_FROM: &str = "Bip01 Neck";

/// Takes off what `entity` wears of the slot 1, else 0 ([`DISARMED_WORN`]), as its disarming does
/// with its hands empty (`0x4E9900`, #2816): out of its [`Arms`] and its [`Equipment`], its model
/// gone, and thrown down from its neck's bone (else its own place) with its values; a living
/// one's values made anew of what it holds and wears then, of which it keeps a share
/// ([`Rearms::disarm`]).
fn disarm_worn(world: &mut bevy::prelude::World, entity: Entity) {
    let Some(arms) = world.get::<Arms>(entity) else {
        return;
    };
    let slot_of = |category: u8| opensacred_rules::equipment::slot_for(category, |_| false);
    let Some((item, values)) = DISARMED_WORN.iter().find_map(|&slot| {
        arms.worn
            .iter()
            .find(|&&(_, category, _)| slot_of(category) == Some(slot))
            .map(|&(item, _, values)| (item, values))
    }) else {
        return;
    };
    let level = world.get::<ai::Mind>(entity).map_or(1, |mind| mind.level);
    let floor = world
        .get::<crowd::InTheWay>(entity)
        .and_then(|way| way.level);
    let Some(at) = bone_place(world, entity, DISARMED_FROM).or_else(|| {
        world
            .get::<GlobalTransform>(entity)
            .map(GlobalTransform::translation)
    }) else {
        return;
    };
    bevy::log::debug!("npcs: {entity} is disarmed of {item}, which it wore");
    let mut pieces = world.query::<(Entity, &WornBy, &Piece)>();
    let models: Vec<Entity> = pieces
        .iter(world)
        .filter(|(_, by, piece)| by.0 == entity && piece.item == item)
        .map(|(model, ..)| model)
        .collect();
    for model in models {
        world.despawn(model);
    }
    if let Some(mut arms) = world.get_mut::<Arms>(entity)
        && let Some(index) = arms.worn.iter().position(|&(worn, ..)| worn == item)
    {
        arms.worn.remove(index);
    }
    if let Some(mut equipment) = world.get_mut::<Equipment>(entity) {
        equipment
            .0
            .retain(|equip| equip.piece.mount != Mount::Worn || equip.piece.item != item);
    }
    if world.get::<Dies>(entity).is_none() {
        world.entity_mut(entity).insert(Rearms::disarm(None));
    }
    if let Err(error) = world.run_system_cached_with(
        crate::drops::throw_disarmed,
        ((item, level, values), (at, floor)),
    ) {
        bevy::log::warn!("npcs: {entity}'s worn item is not thrown down: {error}");
    }
}

/// Where the bone `bone` of `entity`'s own skeleton is, not of a model it holds or wears.
fn bone_place(world: &bevy::prelude::World, entity: Entity, bone: &str) -> Option<Vec3> {
    let mut stack = vec![entity];
    while let Some(next) = stack.pop() {
        if next != entity && world.get::<Piece>(next).is_some() {
            continue;
        }
        if world
            .get::<Name>(next)
            .is_some_and(|name| name.as_str() == bone)
        {
            return world
                .get::<GlobalTransform>(next)
                .map(GlobalTransform::translation);
        }
        stack.extend(world.get::<Children>(next).into_iter().flatten());
    }
    None
}

/// Changes the weapons of each NPC that rearms ([`Rearms`]): the items it puts away leave its
/// hands and their models go, the new item, made at its level (`0x5D5080`) unless it comes with
/// its values or was held before, goes into its hand with its model, what it puts on that goes
/// in no hand is worn, and its attack, defence, damage and resistances, its hits' share, its
/// clips by the class of what it holds (`motions::Slots::stance`, `attacks`) and whether it
/// fights at range are made anew, a disarmed one keeping a share of its values alone.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn rearm(
    mut commands: Commands,
    (npcs, rules): (Res<Npcs>, Res<fight::Rules>),
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
        let before = arms.holding.clone();
        let gone = |hand: Hand| rearms.away.contains(&hand);
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
                .retain(|item| !item.piece.mount.handed() || !gone(item.piece.hand));
        }
        let grade = mind.level;
        let mut equips = Vec::new();
        for &(item, hand, given) in &rearms.takes {
            let Some(gear) = npcs.gear.get(&item) else {
                continue;
            };
            // An item it held before keeps its values (an `EquipNPC`'s, #2756).
            let kept = before
                .iter()
                .find(|(held, _)| held.item == item)
                .and_then(|&(_, values)| values);
            let values = given.or(kept).or_else(|| {
                let (x, y) = paths::cell_of(transform.translation.x, transform.translation.z);
                let mut dice = spawning::Rng::of(
                    u32::try_from(x).unwrap_or(0),
                    u32::try_from(y).unwrap_or(0),
                    usize::try_from(item).unwrap_or(0),
                );
                npcs.items
                    .make(item, grade, (rules.difficulty(), 0), &mut dice)
            });
            let held = InHand {
                item,
                category: gear.category,
                kind: gear.kind,
                hand,
            };
            arms.holding.push((held, values));
            if let Some(model) = &gear.model {
                equips.push(Equip {
                    piece: Piece {
                        item,
                        model: model.clone(),
                        texture: gear.texture.clone(),
                        hand,
                        mount: Mount::held(gear.category),
                    },
                    gltf: assets.load(model.clone()),
                    texture: gear.texture.as_ref().map(|t| load_texture(&assets, t)),
                    ready: true,
                    waits: 0,
                });
            }
        }
        // What it puts on that goes in no hand, with its values given, else made at its level,
        // its model on its skeleton.
        for (item, category, held, given) in &rearms.wears {
            let values = given.or_else(|| {
                let (x, y) = paths::cell_of(transform.translation.x, transform.translation.z);
                let mut dice = spawning::Rng::of(
                    u32::try_from(x).unwrap_or(0),
                    u32::try_from(y).unwrap_or(0),
                    usize::try_from(*item).unwrap_or(0),
                );
                npcs.items
                    .make(*item, grade, (rules.difficulty(), 0), &mut dice)
            });
            arms.worn.push((*item, *category, values));
            if let Some(held) = held {
                equips.push(Equip {
                    piece: held.piece(),
                    gltf: assets.load(held.model.clone()),
                    texture: held.texture.as_ref().map(|t| load_texture(&assets, t)),
                    ready: true,
                    waits: 0,
                });
            }
        }
        match equipment {
            Some(mut equipment) => equipment.0.extend(equips),
            None if !equips.is_empty() => {
                commands.entity(entity).insert(Equipment(equips));
            }
            None => {}
        }
        let (values, hits) = arms.fight(grade, rank.0, &npcs.fight);
        // Its pace by what it holds now, and its walk and run by it.
        repace(&mut mind, &mut npc, values.0.pace_factor());
        *combat = values;
        // A disarmed one keeps a share of them (`0x4E9900`, #2298).
        if rearms.disarmed {
            combat.0 = opensacred_rules::fight::disarmed(combat.0);
        }
        npc.hits = hits;
        let in_hands: Vec<InHand> = arms.holding.iter().map(|&(held, _)| held).collect();
        npc.hands_empty = in_hands.is_empty();
        mind.ranged = ranged_of(&in_hands);
        // Its clips by what it holds now: its graph is made anew (`animate`).
        if let Some(slots) = npcs.slots.get(&arms.model) {
            npc.names = ClipNames::of(slots, class_of(&in_hands)).of_creature(mind.creature);
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
#[derive(Component, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Plays {
    pub instrument: usize,
    pub on: bool,
}

impl MapEntities for Plays {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, _: &mut E) {}
}

impl crate::doings::Saves for Plays {
    type Saved = Plays;

    fn saved(&self, _: &crate::doings::Moment) -> Option<Self> {
        Some(*self)
    }

    fn again(saved: Self, _: &crate::doings::Moment) -> Option<Self> {
        Some(saved)
    }
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
            piece: Piece {
                item: 0,
                model: model.clone(),
                texture: texture.clone(),
                hand: Hand::Right,
                mount: Mount::Hand,
            },
            gltf: assets.load(model.clone()),
            texture: texture.as_ref().map(|t| load_texture(&assets, t)),
            ready: true,
            waits: 0,
        }]));
    }
}

/// An NPC that plays its talk once where it stands (`*_TALK_A`, slot 177; `ai`'s meetings).
#[derive(Component)]
pub struct Talks;

/// Notes the quests' items and the scripts' other lying items that the hero took, which their
/// sectors place no more (#1256, #1296).
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
            .init_resource::<DyingDice>()
            .add_message::<ArtHit>()
            .init_resource::<AreaChanges>()
            .init_resource::<Morphs>()
            .init_resource::<ScriptAttacks>()
            .add_observer(retexture)
            .add_observer(equipment_ready)
            .add_observer(keep)
            .add_observer(named)
            .add_systems(
                Update,
                tick.run_if(bevy::prelude::in_state(crate::game::AppState::InGame)),
            )
            .add_observer(hear_again)
            .add_systems(
                Update,
                (
                    script_records,
                    taken_items,
                    (region_changes, morph).chain(),
                    script_states,
                    script_healths,
                    (hero_riding, place).chain(),
                    rearm,
                    animate,
                    door_clips,
                    (take_steps, take_hero_steps, cinema_holds).chain(),
                    (walk, carry_riders).chain(),
                    put,
                    swing,
                    instrument,
                    (hold_shots, strike).chain(),
                    die,
                    (script_clips, turn_emerged, big_demons_stand).chain(),
                    (stance, paced).chain(),
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

    use bevy::animation::{ActiveAnimation, RepeatAnimation};
    use bevy::prelude::Vec2;
    use opensacred_data::netscript::{Function, Operand};

    use super::*;

    #[test]
    fn a_states_butterflies_are_its_last_0x7f_above_0() {
        use opensacred_data::netscript::Operand;
        let op = |tag, value| Operand { tag, value };
        assert_eq!(butterflies(&[op(0x7F, Value::I32(20))]), Some(20));
        assert_eq!(
            butterflies(&[op(0x7F, Value::U16(5)), op(0x7F, Value::U8(3))]),
            Some(3)
        );
        assert_eq!(butterflies(&[op(0x7F, Value::I32(0))]), None);
        assert_eq!(butterflies(&[op(0x7F, Value::I32(-4))]), None);
        assert_eq!(butterflies(&[op(0x80, Value::I32(20))]), None);
    }

    #[test]
    fn a_death_beside_a_soulcatcher_on_its_level_puts_its_stamp_back() {
        let mut app = App::new();
        app.add_plugins(bevy::prelude::MinimalPlugins)
            .init_resource::<hero::GameSpeed>()
            .init_resource::<DyingDice>()
            .add_systems(Update, die);
        let mut effects = crate::effects::Effects::default();
        effects.add(
            opensacred_rules::fight::Effect::new(opensacred_rules::arts::SOULCATCHER, 1.0),
            40.0,
        );
        app.world_mut()
            .spawn((Transform::default(), hero::Body::standing(), effects));
        let dying = |app: &mut App, x: f32, level: Level| {
            app.world_mut()
                .spawn((
                    Transform::from_xyz(x, 0.0, 0.0),
                    Npc {
                        gltf: Handle::default(),
                        model: String::new(),
                        names: ClipNames::default(),
                        graph: None,
                        player: None,
                        hands_empty: true,
                        instrument: None,
                        hits: Hits {
                            class: 0,
                            share: 1.0,
                        },
                        pace: 1.0,
                    },
                    Dies::new(0),
                    ai::Mind::new(Vec3::ZERO, 1, (false, false), None, [70.0, 180.0], (76, 2)),
                    crowd::InTheWay::of(0, 0, level),
                ))
                .id()
        };
        // A goblin warrior of the size 50 within 550 of him, one beyond, one on another level.
        let near = dying(&mut app, 540.0, None);
        let far = dying(&mut app, 560.0, None);
        let upstairs = dying(&mut app, 100.0, Some((0, 0, 1)));
        app.update();
        let back = |entity| {
            app.world()
                .get::<crate::bodies::PutBack>(entity)
                .map(|back| back.0)
        };
        assert_eq!(back(near), Some(18.0));
        assert_eq!(back(far), None);
        assert_eq!(back(upstairs), None);
    }

    #[test]
    fn the_small_spider_plays_its_walk_for_every_action() {
        let names = ClipNames {
            stand: Some("STAND".to_owned()),
            walk: Some("WALK".to_owned()),
            run: Some("RUN".to_owned()),
            attacks: vec!["ATTACK".to_owned()],
            dying: vec![(173, "DIE".to_owned())],
            attack_slot: 79,
            ..ClipNames::default()
        };
        let spider = names.clone().of_creature(SMALL_SPIDER);
        let walk = Some("WALK".to_owned());
        assert_eq!(
            (&spider.stand, &spider.run, &spider.stance),
            (&walk, &walk, &walk)
        );
        assert_eq!(
            (spider.attacks, spider.dying),
            (vec!["WALK".to_owned()], vec![(173, "WALK".to_owned())])
        );
        assert_eq!((spider.attack_slot, spider.defend), (27, Some(27)));
        // Any other creature's as they are.
        assert_eq!(names.clone().of_creature(70).stand, names.stand);
    }

    #[test]
    fn two_weapons_halve_the_hits_and_hit_twice() {
        // A weapon in each hand, the left no shield, halves the hits' share (`0x4EA3F0`, #2748)
        // and is the class 8, by which a clip of one hit point hits twice (`0x4EA1F0`).
        let held = |hand, category, kind| InHand {
            item: 1,
            category,
            kind: Some(kind),
            hand,
        };
        let blades = [held(Hand::Left, WEAPON, 1), held(Hand::Right, WEAPON, 1)];
        assert_eq!(
            class_of(&blades),
            opensacred_rules::fight::TWO_WEAPONS_CLASS
        );
        assert!(two_weapons_of(&blades, class_of(&blades)));
        let shielded = [held(Hand::Left, SHIELD, 0), held(Hand::Right, WEAPON, 1)];
        assert!(!two_weapons_of(&shielded, class_of(&shielded)));
        let one = [held(Hand::Right, WEAPON, 1)];
        assert!(!two_weapons_of(&one, class_of(&one)));
        let left = [held(Hand::Left, WEAPON, 1)];
        assert!(!two_weapons_of(&left, class_of(&left)));
        // Two items of no class are the class 0, yet two weapons.
        let plain = [held(Hand::Left, WEAPON, 0), held(Hand::Right, WEAPON, 0)];
        assert_eq!(class_of(&plain), 0);
        assert!(two_weapons_of(&plain, 0));
    }

    #[test]
    fn a_placed_objects_height_raises_it_as_the_originals_view_does() {
        // The wall torch's flame over the stairs of 58,1: its record's point, 120 high (#2728).
        let spot = Vec2::new(199_840.0, 6_155.0);
        let placed = raised(spot, 120.0);
        // On its record's point, not moved along the line of sight.
        assert_eq!((placed.x, placed.z), (spot.x, spot.y));
        // 1.7173 pixels a unit at zoom 1: a world unit up spans cos 30° of the ground's pixels.
        let pixels = placed.y * iso::PIXELS_PER_UNIT * (std::f32::consts::FRAC_PI_6).cos();
        assert!((pixels - 120.0 * 1.717_300_2).abs() < 1e-2, "{pixels}");
    }

    #[test]
    fn an_item_goes_onto_its_holders_own_bone_not_an_items_socket() {
        use bevy::ecs::system::SystemState;
        use bevy::ecs::world::World;
        let mut world = World::new();
        let piece = Piece {
            item: 7905,
            model: "shield.glb".into(),
            texture: None,
            hand: Hand::Left,
            mount: Mount::Forearm,
        };
        // The Vampiress: no socket of her own, her shield's under her left forearm.
        let holder = world.spawn(Name::new("vlady_d")).id();
        let hand = world
            .spawn((Name::new("Bip01 R Hand"), ChildOf(holder)))
            .id();
        let forearm = world
            .spawn((Name::new("Bip01 L Forearm"), ChildOf(holder)))
            .id();
        let shield = world.spawn((piece, ChildOf(forearm))).id();
        world.spawn((Name::new("Bone_weapon_01"), ChildOf(shield)));
        type Bones = (
            Query<'static, 'static, &'static Children>,
            Query<'static, 'static, &'static Name>,
            Query<'static, 'static, (), With<Piece>>,
        );
        let mut state: SystemState<Bones> = SystemState::new(&mut world);
        let (children, names, pieces) = state.get(&world).expect("the queries");
        let bone = |wanted| own_bone(holder, wanted, (&children, &names, &pieces));
        assert_eq!(bone("Bone_weapon_01"), None);
        assert_eq!(bone("Bip01 R Hand"), Some(hand));
        // A hero's own socket under its hand is found before the shield's.
        let own = world
            .spawn((Name::new("Bone_weapon_01"), ChildOf(hand)))
            .id();
        let (children, names, pieces) = state.get(&world).expect("the queries");
        assert_eq!(
            own_bone(holder, "Bone_weapon_01", (&children, &names, &pieces)),
            Some(own)
        );
    }

    #[test]
    fn a_queue_steers_a_walk_short_of_its_target_only() {
        let (far, leg) = (Vec3::new(5000.0, 0.0, 0.0), Vec3::new(1400.0, 0.0, 0.0));
        let mut walk = Walk::run(far, None, 276.0);
        let planned =
            |to: Vec3| paths::Planned::new(to, to, paths::Course::Straight(Vec2::ZERO), Vec3::ZERO);
        walk.course = Some(planned(leg));
        walk.steer();
        assert!(walk.course.is_none());
        walk.course = Some(planned(far));
        walk.steer();
        assert!(walk.course.is_some());
    }

    #[test]
    fn a_piece_carried_already_is_taken_out_of_the_equipment_once() {
        let piece = |model: &str, hand| Piece {
            item: 0,
            model: model.into(),
            texture: None,
            hand,
            mount: Mount::Worn,
        };
        let equip = |piece| Equip {
            piece,
            gltf: Handle::default(),
            texture: None,
            ready: true,
            waits: 0,
        };
        let helmet = piece("helmet.glb", Hand::Right);
        let mut equipment = Equipment(vec![
            equip(helmet.clone()),
            equip(piece("sword.glb", Hand::Right)),
        ]);
        assert!(equipment.take(&helmet));
        assert!(!equipment.take(&helmet));
        // Another hand is another piece.
        assert!(!equipment.take(&piece("sword.glb", Hand::Left)));
        assert_eq!(equipment.0.len(), 1);
    }

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
    fn a_models_heading_is_read_back_from_its_rotation() {
        // Each heading as the turned model faces it, within (−180, 180].
        for turned in [0.0_f32, 20.0, 45.0, 135.0, 200.0, 330.0] {
            let rotation = hero::facing(heading(turned));
            let back = heading_of(rotation);
            let apart = (back - turned).rem_euclid(360.0);
            assert!(apart.min(360.0 - apart) < 1e-3, "{turned}: {back}");
            assert!(back > -180.0 && back <= 180.0, "{turned}: {back}");
        }
    }

    #[test]
    fn a_creature_comes_back_facing_as_it_faced() {
        let centre = paths::centre((5617, 2258));
        let at = Transform::from_xyz(centre.x, 0.0, centre.y)
            .with_rotation(hero::facing(heading(200.0)));
        let health = fight::Health {
            current: 40,
            most: 72,
            level: 3,
        };
        let stood = stood_of((&at, None), Some(&health), (false, false), None).unwrap();
        assert!((stood.heading - -160.0).abs() < 1e-3, "{}", stood.heading);
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
        // Without its cell, it stands in the cell under its world position (#2069).
        assert_eq!(world_cell(&ops), Some((13, 20, 0)));
        assert_eq!(world_cell(&ops[..2]), None);
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
    fn an_equip_npc_puts_its_items_on_the_creature_made_before_it() {
        let op = |tag, value| Operand { tag, value };
        let named = |kind, name: &str, rest: &[Operand]| {
            let mut ops = vec![op(0x01, Value::Str(name.into()))];
            ops.extend_from_slice(rest);
            Record {
                kind,
                body: Body::Operands(ops),
            }
        };
        let items = |items: &[i32]| -> Vec<Operand> {
            items.iter().map(|&i| op(0x02, Value::I32(i))).collect()
        };
        // The Daemon's quest 209's captive and the abbess of `help47`, her name cased otherwise
        // than her `EquipNPC`s' (`0x481D80`, #2752); an `EquipNPC` before its creature, or of a
        // creature made by no record here, puts nothing.
        let records = [
            named(EQUIP_NPC, "res:17436", &items(&[4010])),
            named(CREATE_NPC, "res:17436", &items(&[132])),
            named(EQUIP_NPC, "res:17436", &items(&[4015])),
            named(EQUIP_NPC, "res:17436", &items(&[4011, 4013])),
            named(CREATE_NPC, "Res:18245", &items(&[132])),
            named(EQUIP_NPC, "res:18245", &items(&[4017])),
            named(EQUIP_NPC, "res:1", &items(&[4012])),
        ];
        let records: Vec<&Record> = records.iter().collect();
        let made: HashMap<*const Record, usize> = [(1, 0), (4, 1)]
            .iter()
            .map(|&(r, index)| (std::ptr::from_ref(records[r]), index))
            .collect();
        let mut placements = vec![Placement::default(); 2];
        let equips = set_states(&records, &made, &mut placements);
        assert_eq!(
            equips,
            [(0, vec![4015]), (0, vec![4011, 4013]), (1, vec![4017])]
        );
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
        let named = HashMap::from([(
            "novizin1".to_owned(),
            quests::Place::of(&[3237, 2514]).unwrap(),
        )]);
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
    fn a_set_group_state_names_its_nameless_members_and_lets_them_rise() {
        let op = |tag, value| Operand { tag, value };
        // A key and the rising again given, as `RegionEnter`'s and quest 6001's records do.
        let state = GroupState::of(
            &[
                op(GROUP, Value::I32(5003)),
                op(0x01, Value::I32(17_190)),
                op(RISES, Value::I32(1)),
            ],
            &[],
        );
        assert_eq!((state.key, state.rises), (Some(17_190), Some(true)));
        let mut nameless = Placement::default();
        state.apply(&mut nameless);
        assert_eq!(nameless.called, Some(17_190));
        assert!(nameless.rises);
        // A member of a name of its own keeps it.
        let mut named = Placement {
            name: Some("schmied".into()),
            ..Placement::default()
        };
        state.apply(&mut named);
        assert_eq!(named.called, None);
        let mut called = Placement {
            called: Some(5),
            ..Placement::default()
        };
        state.apply(&mut called);
        assert_eq!(called.called, Some(5));
        // A `0x6B` of 0 takes the rising away; none leaves it.
        let takes = GroupState::of(
            &[op(GROUP, Value::I32(6001)), op(RISES, Value::I32(0))],
            &[],
        );
        assert_eq!((takes.key, takes.rises), (None, Some(false)));
        takes.apply(&mut nameless);
        assert!(!nameless.rises);
        let leaves = GroupState::of(&[op(GROUP, Value::I32(6001))], &[]);
        assert_eq!(leaves.rises, None);
        nameless.rises = true;
        leaves.apply(&mut nameless);
        assert!(nameless.rises);
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
        // Its hostility does (#1841): quest 1703's noble turned hostile, his side as his
        // placement's would be.
        let hostile = State::of(
            &[
                op(0x01, Value::Str("res:17219".into())),
                op(0x08, Value::Marker),
                op(0x12, Value::Marker),
            ],
            &[],
        );
        assert!(hostile.restates() && hostile.hostile == Some(true));
        assert!(minds::hostile_to_heroes(
            minds::stated_side(3, 667, hostile.word, true),
            667
        ));
    }

    #[test]
    fn a_set_npc_state_gives_a_creature_made_before_its_flags_word() {
        let op = |tag, value| Operand { tag, value };
        // The arena's dark gladiator (#2426): made hostile, then `0x08` and `0x12` as the hero
        // steps into the arena, the flags' bit 1 by which the hero's look wakes it to fight.
        let attack = State::of(
            &[
                op(0x01, Value::Str("res:17087".into())),
                op(0x08, Value::Marker),
                op(0x12, Value::Marker),
            ],
            &[],
        );
        let mut placement = Placement {
            creature: (689, 2, 0x2000),
            side: 2,
            ..Placement::default()
        };
        attack.restate_flags(&mut placement);
        assert_eq!(placement.creature.2, 0x2001);
        assert_eq!(placement.side, 2);
        // A record of its flags alone restates a creature too.
        assert!(State::of(&[op(0x12, Value::Marker)], &[]).restates());
        // A trader's `0x800` takes its trades, its word as it was, rather than standing in it.
        let leave = State::of(&[op(0x43, Value::Marker)], &[]);
        assert_eq!(leave.flags_of(0x1, 4), (0x1, 0x0, true));
        assert_eq!(leave.flags_of(0x1, 0), (0x800, 0x800, false));
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
        // Its `0x6B` sets `+0x2B7` bit 8 above 0 and clears it for 0, on its placement too
        // (#2459): quest 59 hires Shareefa with `0x4B` 0 and `0x6B` 1.
        let rise = |value| Operand {
            tag: RISES,
            value: Value::I32(value),
        };
        let hires = State::of(&[op(0x0A), op(0x4B), rise(1)], &[]);
        assert_eq!(hires.rises, Some(true));
        assert_eq!(State::of(&[rise(0)], &[]).rises, Some(false));
        assert_eq!(State::of(&[op(0x4B)], &[]).rises, None);
        let mut placement = Placement::default();
        hires.restate_placement(&mut placement);
        assert!(placement.rises);
        State::of(&[rise(0)], &[]).restate_placement(&mut placement);
        assert!(!placement.rises);
    }

    #[test]
    fn a_set_npc_states_0xa1_marks_its_creature_a_vision_for_good() {
        // Quest 56's `Game_over` marks Vilya, of the type 224, a vision (#3064), both her
        // placement and, made before, the creature in the world.
        let marker = |tag| Operand {
            tag,
            value: Value::Marker,
        };
        let marks = State::of(&[marker(VISION)], &[]);
        assert!(marks.vision && marks.restates());
        let mut placement = Placement::default();
        marks.apply(&mut placement);
        assert!(placement.vision);
        // A state with no `0xA1` leaves the mark.
        let hostile = State::of(&[marker(0x08)], &[]);
        assert!(!hostile.vision);
        hostile.restate_placement(&mut placement);
        assert!(placement.vision);
    }

    #[test]
    fn a_derived_weapon_sounds_as_its_parent() {
        let item = |slot, sound| Item {
            slot,
            kind: 2,
            class: 0,
            subclass: 0,
            name: "sword.grn".into(),
            texture: 0,
            sprite: 0,
            byte_03: 0,
            words_08: [0; 2],
            words_14: [0, 0, 0, 0, sound, 0, 0, 0],
            bytes_34: [0; 3],
            byte_57: 0,
            words_58: [0; 10],
        };
        let weapon = |id, parent| opensacred_data::tables::Weapon {
            id,
            parent,
            name: String::new(),
            floats_00: [0.0; 5],
            bytes_14: [0; 4],
            float_18: 0.0,
            bytes_1c: [0; 8],
            words_84: Vec::new(),
        };
        // The Seraphim's starting sword 7901, of the parent 1724, the sword's profile 500; a
        // type with a slot and a profile of its own keeps them.
        let (sword, own) = (item(1724, 500), item(1712, 501));
        let items = HashMap::from([(1724, &sword), (1712, &own)]);
        let weapons = [weapon(7901, 1724), weapon(1724, 0), weapon(1712, 1724)];
        let gear = gear_table(
            &items,
            &weapons,
            &HashMap::new(),
            (&HashMap::new(), &HashMap::new()),
            &[7901, 1712],
        );
        assert_eq!(gear[&7901].sound, 500);
        assert_eq!(gear[&1712].sound, 501);
    }

    #[test]
    fn a_weapon_of_no_profile_sounds_as_a_weapon_of_its_family() {
        let item = |slot, sound| Item {
            slot,
            kind: 2,
            class: 0,
            subclass: 0,
            name: "sword.grn".into(),
            texture: 0,
            sprite: 0,
            byte_03: 0,
            words_08: [0; 2],
            words_14: [0, 0, 0, 0, sound, 0, 0, 0],
            bytes_34: [0; 3],
            byte_57: 0,
            words_58: [0; 10],
        };
        let weapon = |id, family: u16| opensacred_data::tables::Weapon {
            id,
            parent: 0,
            name: String::new(),
            floats_00: [0.0; 5],
            bytes_14: [0; 4],
            float_18: 0.0,
            bytes_1c: [0; 8],
            words_84: vec![family << 8],
        };
        // The C library's first draw after `srand(1242)`: 4094, the fifth of the swords' 1729;
        // after `srand(1711)`, 5626, the second, 1725.
        assert_eq!([first_rand(1242), first_rand(1711)], [4094, 5626]);
        let (daemons, other, quiet) = (item(1242, 0), item(1711, 0), item(1999, 0));
        let (fifth, second) = (item(1729, 505), item(1725, 502));
        let items = HashMap::from([
            (1242, &daemons),
            (1711, &other),
            (1999, &quiet),
            (1729, &fifth),
            (1725, &second),
        ]);
        let weapons = [weapon(1242, 0), weapon(1711, 0), weapon(1999, 3)];
        let gear = gear_table(
            &items,
            &weapons,
            &HashMap::new(),
            (&HashMap::new(), &HashMap::new()),
            &[1242, 1711, 1999],
        );
        assert_eq!(gear[&1242].sound, 505);
        assert_eq!(gear[&1711].sound, 502);
        // A family of no list stays silent.
        assert_eq!(gear[&1999].sound, 0);
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
            switches: Vec::new(),
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
    fn an_equip_npc_takes_a_two_handed_or_ranged_weapon_out_of_the_hands_first() {
        let (mut makers, _) = soldiers();
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
        for (slot, name, category, kind) in [
            (1729, "sword.grn", WEAPON, Some(1)),
            (1726, "sword_2handed.grn", WEAPON, Some(2)),
            (1733, "bow_long.grn", WEAPON, Some(BOW)),
            (4067, "special.grn", WEAPON, Some(1)),
            (1200, "shield_kite.grn", SHIELD, None),
            (3852, "gladiator_metal_02.grn", 6, None),
        ] {
            makers.items.insert(slot, item(slot, name, category));
            if let Some(kind) = kind {
                makers.kinds.insert(slot, kind);
            }
        }
        // `0x481D80`: before each item, a weapon in a hand of the kinds 2, 3, 4, 7 or 12, or of
        // the types 4067 to 4072, is taken out (`0x5CECD0`, #2756); the item goes after the others.
        assert_eq!(makers.equipped(&[1726], &[1729]), [1729]);
        assert_eq!(makers.equipped(&[1733], &[1729]), [1729]);
        assert_eq!(makers.equipped(&[4067], &[1729]), [1729]);
        // A one-handed weapon, a shield and an armour stay.
        assert_eq!(makers.equipped(&[1729], &[1200]), [1729, 1200]);
        assert_eq!(makers.equipped(&[3852, 1726], &[1729]), [3852, 1729]);
        // Each item in turn: the bow given first goes as the shield comes, and as the sword does.
        assert_eq!(makers.equipped(&[1729], &[1733, 1200]), [1729, 1200]);
        assert_eq!(makers.equipped(&[], &[1733, 1729]), [1729]);
        assert!(!taken_out(1729, WEAPON, Some(1)) && !taken_out(1200, SHIELD, None));
        assert!(taken_out(4072, WEAPON, None) && !taken_out(4073, WEAPON, None));
        assert!(taken_out(1, WEAPON, Some(12)) && !taken_out(1, WEAPON, Some(5)));
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
        // Both of a `Weapon.pak` record, which `0x434070` passes (#3065).
        makers.kinds.insert(6001, 0);
        makers.kinds.insert(5218, 0);
        let signpost = Item {
            subclass: UNTAKEABLE,
            ..item(4993, "wegweiser_01.grn", 15)
        };
        makers.items.insert(4993, signpost.clone());
        makers.models.insert(
            "WEGWEISER_01.GRN".to_owned(),
            "models/wegweiser_01.glb".to_owned(),
        );
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
        // Without `0x8E` it is no quest's item lying, whose taking runs its function all the
        // same (quest 10254's note, #2891), and a chest, which nobody picks up, none either.
        let note = made(6001, std::slice::from_ref(&take));
        assert_eq!(note.quest, None);
        assert!(note.lies);
        assert_eq!(note.taking.as_deref(), Some("od_15001"));
        assert_eq!(made(6001, &[]).taking, None);
        assert_eq!(
            made(5218, &[take, op(QUEST_ITEM, Value::Marker)]).quest,
            None
        );
        let of = |category| item(1, "item.grn", category);
        assert!(takeable(&of(5)) && takeable(&of(15)) && takeable(&of(33)));
        assert!(!takeable(&of(4)) && !takeable(&of(10)) && !takeable(&of(12)));
        assert!(!takeable(&of(26)));
        // A plain item of a `CreateObj` lies to be picked up, a signpost not (#1476).
        assert!(made(6001, &[]).lies);
        assert!(!takeable(&signpost));
        let sign = made(4993, &[]);
        assert!(sign.object && !sign.lies && sign.quest.is_none());
        // An item's type sinks under a liquid's surface, a chest's not, nor a type of no
        // `Weapon.pak` record (`0x62AE90`, #3065).
        assert!(made(6001, &[]).sinks);
        assert!(!made(5218, &[]).sinks);
        assert!(!sign.sinks);
        // The tag `0x3F` gives a lying item straight into the backpack (#3543); without it, or
        // on a signpost, none goes.
        let given = made(6001, &[op(TO_PACK, Value::Marker)]);
        assert!(given.lies && given.to_pack);
        assert!(!made(6001, &[]).to_pack);
        let posted = made(4993, &[op(TO_PACK, Value::Marker)]);
        assert!(posted.to_pack && !posted.lies);
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
        // `0x8D` makes it peaceful though `0x08` comes before, and emerges (#2461), quest 74's
        // Sakkara demon's tags.
        let hostile = made(&[op(0x08, Value::Marker)]);
        let emerging = made(&[
            op(0x08, Value::Marker),
            op(0x12, Value::Marker),
            op(EMERGES, Value::Marker),
        ]);
        assert!(!hostile.emerges);
        assert!(emerging.emerges);
        assert_eq!(emerging.hostility, Some(false));
        assert_eq!(hostile.hostility, Some(true));
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
    fn creatures_made_at_a_place_of_a_radius_take_cells_of_their_own_about_it() {
        let soldier = Placement {
            cell: (1321, 2254),
            spread: 6,
            ..Placement::default()
        };
        // No sector loaded: every cell drawn is free, the first draw kept, within the radius.
        let heights = Heights::default();
        let mut drawn = HashSet::new();
        let mut cells = Vec::new();
        for index in 0..5 {
            let cell = spread_cell(&soldier, index, (&heights, None), &drawn).unwrap();
            assert!(
                (cell.0 - 1321).abs() <= 6 && (cell.1 - 2254).abs() <= 6,
                "{cell:?}"
            );
            drawn.insert(cell);
            cells.push(cell);
        }
        // The same record at the same cell each time; the five records at five cells.
        assert_eq!(
            spread_cell(&soldier, 0, (&heights, None), &HashSet::new()),
            Some(cells[0])
        );
        assert_eq!(cells.iter().collect::<HashSet<_>>().len(), 5);
        // No radius: none drawn, its record's cell kept.
        let captain = Placement {
            spread: 0,
            ..soldier.clone()
        };
        assert_eq!(spread_cell(&captain, 0, (&heights, None), &drawn), None);
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
            Record {
                kind: DEF_POS,
                body: Body::Operands(vec![
                    op(0x01, Value::Str("posSoldatenDMG".into())),
                    op(0x0B, Value::I32(1321)),
                    op(0x0B, Value::I32(2254)),
                    op(0x0B, Value::I32(6)),
                ]),
            },
            Record {
                kind: DEF_POS,
                body: Body::Operands(vec![
                    op(0x01, Value::Str("posOrkChampion".into())),
                    op(0x0B, Value::I32(2872)),
                    op(0x0B, Value::I32(3181)),
                    op(0x0B, Value::I32(0)),
                    op(0x0B, Value::I32(1)),
                ]),
            },
        ];
        let named = positions(&records);
        assert_eq!(named.len(), 3);
        assert_eq!(named["hauptmann3"].cell, (3349, 2534));
        // A name is on its fourth number's level, about it the radius of its third (#2442); a
        // cell carries its layer.
        let at = |value| position(&[op(0x02, Value::I32(5207)), op(0x04, value)], &named);
        assert_eq!(
            at(Value::Name("HAUPTMANN3".into())),
            Some(((3349, 2534, 0), 0))
        );
        assert_eq!(
            at(Value::Name("possoldatendmg".into())),
            Some(((1321, 2254, 0), 6))
        );
        assert_eq!(
            at(Value::Name("posOrkChampion".into())),
            Some(((2872, 3181, 1), 0))
        );
        assert_eq!(
            at(Value::I32x3([3259, 2569, 1])),
            Some(((3259, 2569, 1), 0))
        );
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
            marked: false,
            cell: (x, 2518),
            layer: 0,
            heading: 0.0,
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
        // Away from the world (#2406): at its own cell where its maker made it already (the
        // start or the play), not for a sector's `Init` that waits to come near.
        let own = (3349, 2534);
        assert_eq!(kept.away(7, own, true), Some(own));
        assert_eq!(kept.away(7, own, false), None);
        kept.spawned(&goblin);
        assert_eq!(kept.again(7), Again::Not);
        assert_eq!(kept.away(7, own, true), None);
        kept.gone(&goblin, Some(stood(3300, 40)));
        assert_eq!(kept.again(7), Again::AtStood(stood(3300, 40)));
        assert_eq!(kept.away(7, own, true), Some((3300, 2518)));
        assert_eq!(kept.away(7, own, false), Some((3300, 2518)));
        kept.spawned(&goblin);
        kept.gone(&goblin, None);
        assert_eq!(kept.again(7), Again::Not);
        assert_eq!(kept.away(7, own, true), None);
    }

    #[test]
    fn a_body_that_its_dialogue_holds_lies_again_where_it_fell() {
        let health = fight::Health {
            current: 539,
            most: 539,
            level: 11,
        };
        let centre = paths::centre((5617, 2258));
        let at = Transform::from_xyz(centre.x, 0.0, centre.y);
        let stood = |dying, held| stood_of((&at, None), Some(&health), (dying, held), None);
        // Living, it stands with its health; dead, its body lies with none where its dialogue
        // holds it, and is killed for good where none does (#2457).
        let lying = Stood {
            marked: false,
            cell: (5617, 2258),
            layer: 0,
            heading: heading_of(Quat::IDENTITY),
            health: fight::Health {
                current: 0,
                ..health
            },
        };
        assert_eq!(stood(false, true), Some(Stood { health, ..lying }));
        assert_eq!(stood(true, true), Some(lying));
        assert_eq!(stood(true, false), None);
        assert!(lying.lies());
        // A kept one's body comes back lying with its sector, lives nowhere away from the world
        // and lies away from it; once its sector placed it again, it lies there.
        let shareefa = Placed {
            at: (2, 67),
            source: Source::Kept(5),
        };
        let mut kept = Kept::default();
        kept.spawned(&shareefa);
        kept.gone(&shareefa, Some(lying));
        assert_eq!(kept.again(5), Again::AtStood(lying));
        assert_eq!(kept.away(5, (142, 4303), true), None);
        assert_eq!(kept.lies(5), Some((5617, 2258)));
        kept.spawned(&shareefa);
        assert_eq!(kept.lies(5), None);
        // Through a save's bytes, as a load puts it back.
        kept.gone(&shareefa, Some(lying));
        let bytes = postcard::to_stdvec(&kept).unwrap();
        let loaded: Kept = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(loaded.again(5), Again::AtStood(lying));
        // A fill's body is not kept with its sector.
        let wolf = Placed {
            at: (87, 35),
            source: Source::Filled(Box::default()),
        };
        kept.gone(&wolf, Some(lying));
        assert!(kept.filled.is_empty());
    }

    #[test]
    fn a_sector_fills_anew_once_another_region_filled() {
        let stood = |x| Stood {
            marked: false,
            cell: (x, 2518),
            layer: 0,
            heading: 0.0,
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
            let entered = kept.enter(at, Some(region_of(at)), 0.0, region_of);
            (entered.back.len(), entered.fill)
        };
        let (here, there) = ((49, 39), (47, 39));
        let mut kept = Kept::default();
        // Filled at first; a sector of no fills fills nothing.
        assert_eq!(enter(&mut kept, here), (0, true));
        assert!(!kept.enter((50, 39), None, 0.0, region_of).fill);
        // One of its fills went living: it comes back where it stood, and the sector fills
        // nothing.
        kept.gone(&rabbit(here), Some(stood(3150)));
        let entered = kept.enter(here, Some(2), 0.0, region_of);
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
    fn a_save_keeps_the_creatures_of_the_sectors_in_the_world_as_they_would_go() {
        let stood = |x, current| Stood {
            marked: false,
            cell: (x, 2518),
            layer: 0,
            heading: 0.0,
            health: fight::Health {
                current,
                most: 72,
                level: 3,
            },
        };
        let kept_one = |index| Placed {
            at: (51, 39),
            source: Source::Kept(index),
        };
        let (goblin, killed, follower) = (kept_one(7), kept_one(8), kept_one(9));
        let rabbit = Placed {
            at: (51, 39),
            source: Source::Filled(Box::new(Placement {
                model: "rabbit".to_owned(),
                ..Placement::default()
            })),
        };
        // The sector came in and filled; its creatures stand in the world.
        let mut kept = Kept::default();
        assert!(kept.enter((51, 39), Some(2), 0.0, |_| 2).fill);
        for placed in [&goblin, &killed, &follower, &rabbit] {
            kept.spawned(placed);
        }
        // Saved: the goblin and the rabbit stand, the other was killed, and the follower goes
        // with the hero; the game goes on with its own.
        let saved = kept.with_gone([
            (&goblin, Some(stood(3300, 40))),
            (&killed, None),
            (&rabbit, Some(stood(3310, 12))),
        ]);
        assert_eq!(kept.again(7), Again::Not);
        let goblin_made = Placement {
            model: "goblin".to_owned(),
            on_death: DeathFunctions::default(),
            ..Placement::default()
        };
        // The goblin walked under the anim mode, then waited 3 s more (#2890).
        let walk = Queued::Walk {
            to: Vec3::new(10.0, 0.0, 20.0),
            onto: None,
            orders: 1,
            noted: Vec3::ZERO,
        };
        let waits = Queued::Wait {
            other: None,
            until: Until::Time(3.0),
        };
        let creatures = Creatures {
            kept: saved,
            placements: vec![(7, goblin_made)],
            queues: vec![(7, SavedQueue(vec![walk.clone(), waits]))],
            entered: Vec::new(),
        };
        // Through the save's bytes, as a load puts them back.
        let bytes = postcard::to_stdvec(&creatures).unwrap();
        let mut loaded: Creatures = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(loaded, creatures);
        // Its queue on the clock of the run that loads it: the wait's 3 s left from then.
        let (index, queue) = loaded.queues[0].clone();
        assert_eq!(index, 7);
        let queue = queue.loaded(500.0);
        assert_eq!(queue.0[0], walk);
        assert_eq!(
            queue.0[1],
            Queued::Wait {
                other: None,
                until: Until::Time(503.0)
            }
        );
        // A deadline of the run that saves, 10 s in, kept as the seconds left; a cast begun and
        // a `Partikel`'s caster of that run left out.
        let running = ScriptQueue(
            [
                Queued::Wait {
                    other: Some("mona".into()),
                    until: Until::Time(14.0),
                },
                Queued::Cast {
                    cast: crate::creature_casts::ScriptCast::new(9, Entity::from_bits(42), Vec3::X),
                    begun: true,
                },
                Queued::Partikel {
                    at: "mona".into(),
                    caster: Some(Entity::from_bits(42)),
                    numbers: vec![1],
                },
            ]
            .into(),
        );
        let bytes = postcard::to_stdvec(&running.saved(10.0)).unwrap();
        let back = postcard::from_bytes::<SavedQueue>(&bytes).unwrap().0;
        assert_eq!(
            back[0],
            Queued::Wait {
                other: Some("mona".into()),
                until: Until::Time(4.0)
            }
        );
        assert_eq!(
            back[1],
            Queued::Cast {
                cast: crate::creature_casts::ScriptCast::new(9, Entity::PLACEHOLDER, Vec3::X),
                begun: false,
            }
        );
        assert!(matches!(&back[2], Queued::Partikel { caster: None, .. }));
        let kept = &mut loaded.kept;
        assert_eq!(kept.again(7), Again::AtStood(stood(3300, 40)));
        assert_eq!(kept.again(8), Again::Not);
        // The follower stays out of its sector: the save puts it beside the hero.
        assert_eq!(kept.again(9), Again::Not);
        // The rabbit comes back with its sector, which fills nothing then.
        let entered = kept.enter((51, 39), Some(2), 0.0, |_| 2);
        assert!(!entered.fill);
        assert_eq!(entered.back.len(), 1);
        assert_eq!(entered.back[0].0.model, "rabbit");
        assert_eq!(entered.back[0].1, stood(3310, 12));
    }

    #[test]
    fn a_save_keeps_a_creature_emerging_to_emerge_again() {
        // Quest 74's big demon saved in its clip of the slot 154 comes back emerging, to turn
        // hostile as the clip ends (#3075); one turned keeps the mark no more.
        let emerging = Placement {
            creature: (BIG_DEMON, 0, 0),
            side: 3,
            emerges: true,
            ..Placement::default()
        };
        let bytes = postcard::to_allocvec(&emerging).expect("saved");
        let loaded: Placement = postcard::from_bytes(&bytes).expect("loaded");
        assert!(loaded.emerges);
        assert_eq!(loaded, emerging);
    }

    #[test]
    fn a_save_keeps_a_follower_lying_to_rise_with_the_time_it_has_lain() {
        let mut world = bevy::prelude::World::new();
        let hero = world.spawn(hero::Body::standing()).id();
        let mind = ai::Mind::new(
            Vec3::ZERO,
            minds::FOLLOWS,
            (false, true),
            None,
            [70.0, 180.0],
            (697, 3),
        );
        let way = crowd::InTheWay::of(0, 0, None);
        let health = |current| fight::Health {
            current,
            most: 539,
            level: 11,
        };
        let follower = |world: &mut bevy::prelude::World, current| {
            let arms = Arms {
                base: fight::Base::default(),
                plain: false,
                model: String::new(),
                holding: Vec::new(),
                worn: Vec::new(),
                mounted: false,
            };
            let at = Transform::from_xyz(1_000.0, 0.0, 1_000.0);
            let entity = world
                .spawn((arms, at, mind.clone(), way, health(current)))
                .id();
            crate::followers::rehire(world, entity, hero, (false, true));
            entity
        };
        let lay = |world: &mut bevy::prelude::World, entity: Entity, rises: bool| {
            let mut dead = world.entity_mut(entity);
            dead.remove::<(ai::Mind, crowd::InTheWay)>();
            dead.insert((
                Dies::new(0),
                Stashed {
                    mind: mind.clone(),
                    way,
                    lain: 7.5,
                },
            ));
            if rises {
                dead.insert(RisesAgain);
            }
        };
        follower(&mut world, 300);
        let lying = follower(&mut world, 0);
        lay(&mut world, lying, true);
        // One dead for good is no follower of the save.
        let gone = follower(&mut world, 0);
        lay(&mut world, gone, false);
        let saved = following(&mut world);
        assert_eq!(saved.len(), 2);
        let (living, dead) = (&saved[0], &saved[1]);
        assert_eq!((living.rises, living.lain), (false, None));
        assert_eq!(living.stood.health.current, 300);
        // The lying one at its most health, kept to rise, 7.5 s lain.
        assert_eq!((dead.rises, dead.lain), (true, Some(7.5)));
        assert_eq!(dead.stood.health, health(539));
        assert_eq!(dead.flags, mind.flags);
        // A load lays it down again with the time it had lain.
        assert!((Dies::lying(7.5).lain - 7.5).abs() < f32::EPSILON);
        assert!(Dies::lying(7.5).fallen && !Dies::lying(7.5).killed);
    }

    #[test]
    fn a_saved_follower_holds_again_what_it_held() {
        let values = fight::ItemValues::default();
        let bow = InHand {
            item: 1732,
            category: WEAPON,
            kind: Some(BOW),
            hand: Hand::Left,
        };
        // Holding what it was made with, it changes nothing.
        let hands = [(1732, Hand::Left, Some(values))];
        assert_eq!(rearms_for(&hands, &[bow]), None);
        // A sword given into its right hand: that hand takes it, with its values.
        let given = [(1732, Hand::Left, None), (1753, Hand::Right, Some(values))];
        let rearms = rearms_for(&given, &[bow]).unwrap();
        assert_eq!(rearms.away, [Hand::Right]);
        assert_eq!(rearms.takes, [(1753, Hand::Right, Some(values))]);
        // Its bow taken back: the left hand emptied.
        let rearms = rearms_for(&[], &[bow]).unwrap();
        assert_eq!((rearms.away, rearms.takes), (vec![Hand::Left], Vec::new()));
        // Its save reads back as written.
        let following = Following {
            made: Made::Filled(
                (51, 39),
                Box::new(Placement {
                    model: "thief".to_owned(),
                    holding: vec![bow],
                    ..Placement::default()
                }),
            ),
            stood: Stood {
                marked: false,
                cell: (3300, 2518),
                layer: 1,
                heading: 0.0,
                health: fight::Health {
                    current: 30,
                    most: 50,
                    level: 4,
                },
            },
            flags: minds::FOLLOWS,
            side: 3,
            stands: ways::InTheWay {
                stops: true,
                hostile: false,
                creature: true,
            },
            escorted: true,
            listed: true,
            hands: given.to_vec(),
            given: Some(crate::mercenaries::Given::default()),
            effects: None,
            rises: true,
            lain: Some(7.5),
            queue: SavedQueue(vec![Queued::Sound(7), Queued::Call("sprich_15".into())]),
        };
        let bytes = postcard::to_stdvec(&following).unwrap();
        assert_eq!(
            postcard::from_bytes::<Following>(&bytes).unwrap(),
            following
        );
        let summoned = Made::Roams {
            creature: 1026,
            rank: fight::Rank::Common,
            born: Some(41),
        };
        let bytes = postcard::to_stdvec(&summoned).unwrap();
        assert_eq!(postcard::from_bytes::<Made>(&bytes).unwrap(), summoned);
    }

    #[test]
    fn a_strike_saved_keeps_its_hits_and_plays_its_clip_again_at_its_target_by_name() {
        use crate::doings::{Moment, Saves};
        use crate::names::{Id, Loading, Named, Names, NamesPlugin, Refs, Saving, mapped};
        let mut saved = App::new();
        saved.add_plugins(NamesPlugin);
        let hero = saved.world_mut().spawn(Named(Id::Hero)).id();
        let mut strikes = Strikes::at(hero, 2);
        strikes.struck = 1;
        strikes.opens = true;
        let mut refs = Refs::default();
        let kept = mapped(
            strikes.saved(&Moment::default()).unwrap(),
            &mut Saving {
                world: saved.world(),
                refs: &mut refs,
            },
        );
        let bytes = postcard::to_stdvec(&(&refs, &kept)).unwrap();
        let (refs, kept): (Refs, SavedStrikes) = postcard::from_bytes(&bytes).unwrap();
        let mut loaded = App::new();
        loaded.add_plugins(NamesPlugin);
        loaded.world_mut().spawn_batch((0..3).map(|_| ()));
        let hero = loaded.world_mut().spawn(Named(Id::Hero)).id();
        let names = loaded.world().resource::<Names>();
        let back = Strikes::again(
            mapped(
                kept,
                &mut Loading {
                    refs: &refs,
                    names,
                    lost: 0,
                },
            ),
            &Moment::default(),
        )
        .unwrap();
        assert_eq!((back.target, back.choice, back.struck), (hero, 2, 1));
        assert!(back.opens && back.resumed && back.playing.is_none());
        // Its target not back, the strike ends.
        let empty = Names::default();
        let lost = mapped(
            kept,
            &mut Loading {
                refs: &refs,
                names: &empty,
                lost: 0,
            },
        );
        assert!(Strikes::again(lost, &Moment::default()).is_none());
    }

    #[test]
    fn a_save_keeps_a_body_as_it_lies_but_a_dialogues_held_one_and_a_followers() {
        use crate::names::{Id, Named};
        let mut world = bevy::prelude::World::new();
        let centre = paths::centre((3236, 2511));
        let corpse = Corpse {
            creature: 313,
            class: 2,
            dialogue: false,
            level: None,
            hostile: true,
            flags: 1,
        };
        let health = fight::Health {
            current: 0,
            most: 42,
            level: 2,
        };
        let goblin = world
            .spawn((
                Named(Id::Born(12)),
                Placed {
                    at: (50, 39),
                    source: Source::Filled(Box::new(Placement {
                        born: Some(12),
                        ..Placement::default()
                    })),
                },
                Transform::from_xyz(centre.x, 0.0, centre.y),
                corpse,
                Dies::by_creature(2),
                health,
                Stashed {
                    mind: ai::Mind::new(
                        Vec3::ZERO,
                        1,
                        (true, false),
                        None,
                        [70.0, 180.0],
                        (313, 2),
                    ),
                    way: crowd::InTheWay::of(0, 0, None),
                    lain: 0.0,
                },
            ))
            .id();
        // A kept placement's body that its dialogue holds goes with its sector.
        world.spawn((
            Named(Id::Placed(5)),
            Placed {
                at: (50, 39),
                source: Source::Kept(5),
            },
            Transform::from_xyz(centre.x, 0.0, centre.y),
            corpse,
            Dies::new(0),
            health,
            Talk("res:giver".into()),
        ));
        let saved = bodies(&mut world);
        assert_eq!(saved.len(), 1);
        let body = &saved[0];
        assert!(matches!(&body.made, Made::Filled(_, placement) if placement.born == Some(12)));
        assert!(body.stood.lies());
        assert_eq!(
            (body.choice, body.by_creature, body.killed),
            (2, true, true)
        );
        assert_eq!(body.corpse, corpse);
        let bytes = postcard::to_stdvec(&saved).unwrap();
        assert_eq!(
            postcard::from_bytes::<Vec<SavedBody>>(&bytes).unwrap(),
            saved
        );
        assert!(world.get_entity(goblin).is_ok());
    }

    #[test]
    fn an_enter_creature_comes_back_as_it_stood_or_not_once_after_a_load() {
        let stood = Stood {
            cell: (3236, 2511),
            layer: 0,
            heading: 45.0,
            health: fight::Health {
                current: 12,
                most: 30,
                level: 2,
            },
            marked: false,
        };
        let mut entered: HashMap<usize, Option<Stood>> = [(7, Some(stood)), (8, None)].into();
        assert_eq!(entered_again(&mut entered, 7), Again::AtStood(stood));
        assert_eq!(entered_again(&mut entered, 8), Again::Not);
        assert_eq!(entered_again(&mut entered, 9), Again::Anew);
        // At its next coming its `Enter` makes it anew.
        assert_eq!(entered_again(&mut entered, 7), Again::Anew);
        assert_eq!(entered_again(&mut entered, 8), Again::Anew);
    }

    #[test]
    fn a_walk_goes_on_to_its_target_after_a_load_its_course_found_anew() {
        use crate::doings::{Moment, Saves};
        let mut walk = Walk::to(Vec3::new(10.0, 0.0, 20.0), Some((50, 39, 2)), 70.0);
        walk.runs = true;
        walk.comes_on = true;
        walk.within = 40.0;
        walk.onto = Some(None);
        let saved = walk.saved(&Moment::default()).unwrap();
        let bytes = postcard::to_stdvec(&saved).unwrap();
        let back = Walk::again(postcard::from_bytes(&bytes).unwrap(), &Moment::default()).unwrap();
        assert_eq!(back.saved(&Moment::default()), Some(saved));
        assert!(back.course.is_none());
    }

    #[test]
    fn a_sector_fills_anew_no_sooner_than_its_respawns_bound_after_its_gate_passed() {
        let region_of = |(x, _): (u32, u32)| if x < 60 { 1 } else { 2 };
        let wolf = Placed {
            at: (50, 39),
            source: Source::Filled(Box::default()),
        };
        let stood = Stood {
            cell: (3236, 2511),
            layer: 0,
            heading: 0.0,
            health: fight::Health {
                current: 20,
                most: 30,
                level: 2,
            },
            marked: false,
        };
        let mut kept = Kept::default();
        // Its first coming fills it, its gate passing at 0.
        assert!(kept.enter((50, 39), Some(1), 300.0, region_of).fill);
        // Cleared and back within the bound: nothing made, nothing kept.
        kept.clock = 120.0;
        let early = kept.enter((50, 39), Some(1), 300.0, region_of);
        assert!(!early.fill && early.back.is_empty());
        // A living one kept comes back within the bound, its gate left as it was.
        kept.gone(&wolf, Some(stood));
        kept.clock = 250.0;
        let back = kept.enter((50, 39), Some(1), 300.0, region_of);
        assert_eq!((back.fill, back.back.len()), (false, 1));
        // A sector that stood in the world at a save puts back what it kept at its first coming
        // after the load, whatever the gate and the region, and is as any after.
        kept.standing.insert((50, 39));
        kept.gone(&wolf, Some(stood));
        let first = kept.enter((50, 39), Some(1), 300.0, region_of);
        assert_eq!((first.fill, first.back.len()), (false, 1));
        assert!(kept.standing.is_empty());
        // Past the bound it passes: kept a time, its region the last, nothing anew.
        kept.clock = 301.0;
        assert!(!kept.enter((50, 39), Some(1), 300.0, region_of).fill);
        // Another region's fill drops this region's passes too: back at once, it fills anew.
        kept.clock = 320.0;
        assert!(kept.enter((70, 39), Some(2), 300.0, region_of).fill);
        kept.clock = 330.0;
        assert!(kept.enter((50, 39), Some(1), 300.0, region_of).fill);
        // A save keeps the passes and the clock.
        let bytes = postcard::to_stdvec(&kept).unwrap();
        let loaded: Kept = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(loaded, kept);
    }

    #[test]
    fn a_fills_creature_keeps_its_serial_through_its_sectors_going_and_coming() {
        let mut kept = Kept::default();
        let (first, second) = (kept.mint(), kept.mint());
        assert_eq!((first, second, kept.born), (0, 1, 2));
        let wolf = Placed {
            at: (50, 39),
            source: Source::Filled(Box::new(Placement {
                born: Some(second),
                ..Placement::default()
            })),
        };
        let stood = Stood {
            cell: (3236, 2511),
            layer: 0,
            heading: 0.0,
            health: fight::Health {
                current: 20,
                most: 30,
                level: 2,
            },
            marked: false,
        };
        kept.gone(&wolf, Some(stood));
        // A save keeps the count and the serial.
        let bytes = postcard::to_stdvec(&kept).unwrap();
        let mut kept: Kept = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(kept.born, 2);
        let back = kept.enter((50, 39), None, 0.0, |_| 0).back;
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].0.born, Some(second));
        assert_eq!(kept.mint(), 2);
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
    fn a_queued_partikel_makes_its_casts_sights_or_its_system_at_its_turn() {
        let op = |tag, value| Operand { tag, value };
        let name = |n: &str| op(0x01, Value::Str(n.into()));
        let number = |n| op(0x0B, Value::I32(n));
        // The ritual's: under the mode alone, in the second name's queue (#1890).
        let ritual = [
            name("res:17791"),
            name("res:17791"),
            number(-2),
            number(-5),
            number(16_711_680),
            number(1),
        ];
        let partikel = played(PARTIKEL, &ritual, true, 0.0).unwrap();
        assert_eq!(
            (partikel.name.as_str(), partikel.act),
            (
                "res:17791",
                Act::Partikel {
                    at: "res:17791".into(),
                    numbers: vec![-2, -5, 16_711_680, 1]
                }
            )
        );
        assert!(played(PARTIKEL, &ritual, false, 0.0).is_none());
        assert!(played(PARTIKEL, &[name("res:17791"), number(1)], true, 0.0).is_none());
        // At its turn: of `n0` −2 the sights of its code at the creature, else the generic
        // system at its first name's creature.
        let mut tick = WorldTick::default();
        tick.gather(0.2);
        let mut app = App::new();
        app.add_plugins(bevy::prelude::MinimalPlugins)
            .insert_resource(tick)
            .insert_resource(Heights::house())
            .add_message::<sfx::Play>()
            .add_message::<ArtHit>()
            .add_message::<crate::scripts::Partikel>()
            .add_message::<crate::cast_sights::ScriptSights>()
            .add_systems(Update, take_steps);
        let mage = app.world_mut().spawn(Transform::default()).id();
        let steps = [
            Queued::Partikel {
                at: "res:17791".into(),
                caster: Some(mage),
                numbers: vec![-2, -5, 16_711_680, 1],
            },
            Queued::Partikel {
                at: "res:17791".into(),
                caster: Some(mage),
                numbers: vec![-1, -1, 16_711_680, 1],
            },
        ];
        app.world_mut()
            .entity_mut(mage)
            .insert(ScriptQueue(steps.into()));
        app.update();
        let sights: Vec<crate::cast_sights::ScriptSights> = app
            .world()
            .resource::<bevy::ecs::message::Messages<crate::cast_sights::ScriptSights>>()
            .iter_current_update_messages()
            .copied()
            .collect();
        assert_eq!(
            sights,
            [crate::cast_sights::ScriptSights {
                caster: mage,
                code: -5
            }]
        );
        app.update();
        let systems: Vec<(Option<String>, u8)> = app
            .world()
            .resource::<bevy::ecs::message::Messages<crate::scripts::Partikel>>()
            .iter_current_update_messages()
            .map(|partikel| (partikel.at.clone(), partikel.variant))
            .collect();
        assert_eq!(systems, [(Some("res:17791".to_owned()), 1)]);
    }

    #[test]
    fn a_creatures_sound_and_object_wait_its_turn_under_the_anim_mode() {
        let op = |tag, value| Operand { tag, value };
        let name = |n: &str| op(0x01, Value::Str(n.into()));
        // `grab_uw`'s thunder at its creature waits its turn under the mode alone; one of an
        // empty name or of none plays at once (`scripts`).
        let thunder = [name("SOUND_FX_THUNDER01"), name("res:18800")];
        let sound = played(PLAY_SOUND, &thunder, true, 0.0).unwrap();
        assert_eq!(
            (sound.name.as_str(), sound.act),
            ("res:18800", Act::Sound("sound_fx_thunder01".into()))
        );
        assert!(played(PLAY_SOUND, &thunder, false, 0.0).is_none());
        assert!(played(PLAY_SOUND, &[name("line"), name("")], true, 0.0).is_none());
        assert!(played(PLAY_SOUND, &[name("line")], true, 0.0).is_none());
        // Quest 72's objects of the smithy name their smith second; under the mode those of a
        // creature in the world wait its turns, the others are made at once.
        let object = |creature: &str| Record {
            kind: CREATE_OBJ,
            body: Body::Operands(vec![name("Res:18301"), name(creature)]),
        };
        let mode = |kind| Record {
            kind,
            body: Body::Operands(Vec::new()),
        };
        assert_eq!(maker(&object("Res:18237")).as_deref(), Some("res:18237"));
        assert_eq!(maker(&object("HERO")), None);
        let records = [
            object("Res:18237"),
            mode(SET_ANIM_MODE),
            object("Res:18237"),
            object("res:gone"),
            mode(SET_PLAY_MODE),
            object("Res:18237"),
        ];
        assert_eq!(
            deferred_objects(&records, |name| name == "res:18237"),
            HashSet::from([2])
        );
    }

    #[test]
    fn an_attack_of_ecs_teleport_teleports_its_creature_to_its_place() {
        let op = |tag, value| Operand { tag, value };
        let places = HashMap::from([(
            "pos_teleport15031".to_owned(),
            quests::Place::of(&[3100, 2200]).unwrap(),
        )]);
        let teleport = |place: Value| {
            vec![
                op(0x01, Value::Str("Res:17019".into())),
                op(0x04, place),
                op(ART, Value::Str("ECS_TELEPORT".into())),
            ]
        };
        // Quest 1503's `DefPos`, quest 1002's cell of negative numbers.
        assert_eq!(
            phased(&teleport(Value::Str("pos_teleport15031".into())), &places),
            Some(("res:17019".into(), Whither::Cell((3100, 2200, 0))))
        );
        assert_eq!(
            phased(&teleport(Value::I32x3([-1655, -2409, 0])), &places),
            Some(("res:17019".into(), Whither::Cell((-1655, -2409, 0))))
        );
        // Another art, no art or no place: none.
        let meteor = [
            op(0x01, Value::Str("res:1".into())),
            op(0x04, Value::I32x3([1, 2, 0])),
            op(ART, Value::Str("ECS_METEOR".into())),
        ];
        assert_eq!(phased(&meteor, &places), None);
        assert_eq!(
            phased(&[op(0x01, Value::Str("res:1".into()))], &places),
            None
        );
    }

    #[test]
    fn an_attack_of_a_spells_art_orders_its_creatures_cast_at_its_target() {
        let op = |tag, value| Operand { tag, value };
        let name = |n: &str| op(0x01, Value::Str(n.into()));
        let art = |a: &str| op(ART, Value::Str(a.into()));
        // At the hero, named or not; at a creature.
        let places = HashMap::from([(
            "pos_trigarea_eck1_10252".to_owned(),
            quests::Place::of(&[1509, 1216]).unwrap(),
        )]);
        let cast = |ops: &[Operand]| cast_by(ops, &places);
        assert_eq!(
            cast(&[name("Res:1"), name("HERO"), art("ECS_METEOR")]),
            Some(("res:1".into(), 26, CastAt::Hero))
        );
        assert_eq!(
            cast(&[name("res:1"), art("ecs_firestrike")]),
            Some(("res:1".into(), 5, CastAt::Hero))
        );
        assert_eq!(
            cast(&[name("res:1"), name("res:2"), art("ECS_FLASH")]),
            Some(("res:1".into(), 33, CastAt::Named("res:2".into())))
        );
        // Quest 74's demon at a group, quest 10252's boss at a `DefPos`.
        assert_eq!(
            cast(&[name("res:1"), op(0x11, Value::I32(73)), art("ECS_METEOR")]),
            Some(("res:1".into(), 26, CastAt::Group(73)))
        );
        assert_eq!(
            cast(&[
                name("res:1"),
                op(0x04, Value::Str("pos_TrigArea_Eck1_10252".into())),
                art("ECS_METEOR")
            ]),
            Some((
                "res:1".into(),
                26,
                CastAt::Place(Whither::Cell((1509, 1216, 0)))
            ))
        );
        // Quest 10252's own boss, whose name's key is negative, is never found: none.
        assert_eq!(
            cast(&[
                name("res:ENDGEGNER_10252_A"),
                op(0x04, Value::Str("pos_TrigArea_Eck1_10252".into())),
                art("ECS_METEOR")
            ]),
            None
        );
        // The teleport, no art or the hero as the caster: none.
        assert_eq!(cast(&[name("res:1"), art("ECS_TELEPORT")]), None);
        assert_eq!(cast(&[name("res:1"), name("res:2")]), None);
        assert_eq!(cast(&[name("HERO"), art("ECS_WINDSTRIKE")]), None);
    }

    #[test]
    fn a_createnpcs_spell_is_the_readers_number_of_its_name() {
        // Quest 13's priests and black magician, `undweiter`'s priests, quest 6's dark elf.
        assert_eq!(script_spell("ECS_FIRESTRIKE"), 5);
        assert_eq!(script_spell("ECS_METEOR"), 26);
        assert_eq!(script_spell("ecs_firecircle"), 11);
        assert_eq!(script_spell("ECS_ICEBULLETS"), 34);
        assert_eq!(script_spell("ECS_FIRESKIN"), 24);
        // The summon by its first letters, a type after it.
        assert_eq!(script_spell("ECS_BIGSUMMON"), 3);
        assert_eq!(script_spell("ECS_BIGSUMMON 264"), 3);
        // A name the reader does not know, the black magician 08's hellfire, and none.
        assert_eq!(script_spell("ECS_HELLFIRE"), 255);
        assert_eq!(script_spell("ECS_NOSPELL"), 255);
    }

    #[test]
    fn an_attack_finds_a_name_of_a_positive_key_alone() {
        // A number's key is the number; a word's its hash, of the top bit set for these two.
        assert!(attack_finds("res:17810"));
        assert!(attack_finds("res:endgegner1") == attack_finds("res:ENDGEGNER1"));
        assert!(!attack_finds("res:endgegner_10252_a"));
        assert!(!attack_finds("res:endgegner1"));
        assert!(attack_finds("machet1703"));
        // `bum`'s boss at the hero by meteors: none; no `Attack` without an art by it either.
        let op = |tag, value| Operand { tag, value };
        let ops = [
            op(0x01, Value::Str("res:ENDGEGNER1".into())),
            op(0x01, Value::Str("res:17217".into())),
        ];
        assert!(
            script_attacks(&[Record {
                kind: ATTACK,
                body: Body::Operands(ops.to_vec()),
            }])
            .is_empty()
        );
    }

    #[test]
    fn an_attacks_attacker_and_target_follow_their_operands_order() {
        let op = |tag, value| Operand { tag, value };
        let name = |n: &str| op(0x01, Value::Str(n.into()));
        let group = |n| op(0x11, Value::I32(n));
        let attack = |ops| Record {
            kind: ATTACK,
            body: Body::Operands(ops),
        };
        let records = [
            // `malorka_ok`'s group against the hero, quest 1112's creature against a group and
            // a group against it, region 3's groups, quest 20's two creatures.
            attack(vec![group(60000), name("HERO")]),
            attack(vec![name("res:17127"), group(11121)]),
            attack(vec![group(11121), name("res:17127")]),
            attack(vec![group(6000), group(6001)]),
            attack(vec![name("res:17090"), name("res:17089")]),
            // An art's is #1831's, `ECS_NOSPELL` none; a record of one party nothing.
            attack(vec![
                name("res:17038"),
                op(0x04, Value::I32x3([-1655, -2409, 0])),
                op(ART, Value::Str("ECS_TELEPORT".into())),
            ]),
            attack(vec![
                name("res:17309"),
                name("res:17310"),
                op(ART, Value::Str("ECS_NOSPELL".into())),
            ]),
            attack(vec![name("res:1")]),
            // Under the mode, quest 1602's waits its turn.
            Record {
                kind: SET_ANIM_MODE,
                body: Body::Operands(Vec::new()),
            },
            attack(vec![name("res:17201"), name("res:17205")]),
        ];
        let named = |n: &str| Party::Named(n.into());
        let pair = |attacker, target| ScriptAttack { attacker, target };
        assert_eq!(
            script_attacks(&records),
            [
                pair(Party::Group(60000), Party::Hero),
                pair(named("res:17127"), Party::Group(11121)),
                pair(Party::Group(11121), named("res:17127")),
                pair(Party::Group(6000), Party::Group(6001)),
                pair(named("res:17090"), named("res:17089")),
                pair(named("res:17309"), named("res:17310")),
            ]
        );
    }

    #[test]
    fn a_morph_acts_at_once_its_level_reset_or_stated_or_in_its_turn_its_level_kept() {
        let op = |tag, value| Operand { tag, value };
        let morph = |name: &str, creature: i32| Record {
            kind: MORPH,
            body: Body::Operands(vec![
                op(0x01, Value::Str(name.into())),
                op(0x02, Value::I32(creature)),
            ]),
        };
        let mode = |kind| Record {
            kind,
            body: Body::Operands(Vec::new()),
        };
        let stated = Record {
            kind: SET_NPC_STATE,
            body: Body::Operands(vec![
                op(0x01, Value::Str("res:17127".into())),
                op(LEVEL, Value::I32(1000)),
            ]),
        };
        // Quest 1112's `postchange1112`: a `SetNPCState` of a level after it; `fettes`' drake none;
        // the hero and no type nothing; under the mode its turn ([`played`]).
        let records = [
            morph("res:17127", 307),
            stated,
            morph("res:17482", 52),
            morph("HERO", 33),
            morph("res:17482", 0),
            mode(SET_ANIM_MODE),
            morph("res:22008", 309),
            mode(SET_PLAY_MODE),
        ];
        assert_eq!(
            morphs(&records),
            [
                Morph {
                    name: "res:17127".into(),
                    creature: 307,
                    level: Morphed::Stated,
                    stated: vec![vec![
                        op(0x01, Value::Str("res:17127".into())),
                        op(LEVEL, Value::I32(1000)),
                    ]],
                },
                Morph {
                    name: "res:17482".into(),
                    creature: 52,
                    level: Morphed::Reset,
                    stated: Vec::new(),
                },
            ]
        );
        let Body::Operands(ops) = &records[6].body else {
            unreachable!()
        };
        let turn = played(MORPH, ops, true, 0.0).unwrap();
        assert_eq!(
            (turn.name.as_str(), turn.act),
            ("res:22008", Act::Morph(309))
        );
        assert!(played(MORPH, ops, false, 0.0).is_none());
        // At its turn, handed back with its mark: its level kept.
        let mut turned = ops.clone();
        turned.push(op(QUEUED_MORPH, Value::Marker));
        let back = [
            mode(SET_ANIM_MODE),
            Record {
                kind: MORPH,
                body: Body::Operands(turned),
            },
        ];
        assert_eq!(morphs(&back)[0].level, Morphed::Kept);
    }

    #[test]
    fn an_act_obj_opens_or_closes_its_objects_door_at_once_or_in_its_turn() {
        let op = |tag, value| Operand { tag, value };
        let name = |n: &str| op(0x01, Value::Str(n.into()));
        // Quest 1602's killer opens the door in its turn, under the mode; opening is the default.
        let door = [name("res:17201"), name("R16NQ02_Tür_1")];
        let opens = played(ACT_OBJ, &door, true, 0.0).unwrap();
        assert_eq!(
            (opens.name.as_str(), opens.act, opens.queued),
            (
                "res:17201",
                Act::Obj {
                    object: "r16nq02_tür_1".into(),
                    open: true
                },
                true
            )
        );
        // Outside the mode at once; the last of its tags counts.
        let closes = played(
            ACT_OBJ,
            &[
                name("HERO"),
                name("tor"),
                op(0x0F, Value::Marker),
                op(0x27, Value::Marker),
            ],
            false,
            0.0,
        )
        .unwrap();
        assert_eq!(
            (closes.name.as_str(), closes.act, closes.queued),
            (
                HERO,
                Act::Obj {
                    object: "tor".into(),
                    open: false
                },
                false
            )
        );
        // Both names needed.
        assert!(played(ACT_OBJ, &[name("res:17201")], true, 0.0).is_none());
        assert!(played(ACT_OBJ, &[name("res:17201"), name("")], true, 0.0).is_none());
        // Opening unbars its trigger; closing leaves the bar.
        assert_eq!(act_on("tor", true).barred, Some(false));
        assert_eq!(act_on("tor", false).barred, None);
    }

    #[test]
    fn a_queued_object_is_made_and_a_queued_sound_plays_each_at_its_turn() {
        let mut tick = WorldTick::default();
        tick.gather(0.2);
        let mut app = App::new();
        app.add_plugins(bevy::prelude::MinimalPlugins)
            .insert_resource(tick)
            .insert_resource(Heights::house())
            .init_resource::<crate::scripts::WorldRecords>()
            .init_resource::<crate::scripts::ObjStates>()
            .add_message::<sfx::Play>()
            .add_systems(Update, take_steps);
        let object = Record {
            kind: CREATE_OBJ,
            body: Body::Operands(vec![Operand {
                tag: 0x01,
                value: Value::Str("Res:18301".into()),
            }]),
        };
        let steps = [
            Queued::Make(Box::new(object.clone())),
            Queued::Sound(7),
            Queued::Obj {
                object: "tor".into(),
                open: true,
            },
        ];
        let smith = app
            .world_mut()
            .spawn((Transform::default(), ScriptQueue(steps.into())))
            .id();
        let sounds = |app: &App| {
            app.world()
                .resource::<bevy::ecs::message::Messages<sfx::Play>>()
                .iter_current_update_messages()
                .filter(|play| matches!(play.sound, sfx::Of::Id(7)) && play.at.is_none())
                .count()
        };
        // One step a tick: the object handed to the world's records, made as they are.
        app.update();
        assert_eq!(
            app.world().resource::<crate::scripts::WorldRecords>().0,
            [object]
        );
        assert_eq!(sounds(&app), 0);
        // Then the sound, in the middle.
        app.update();
        assert_eq!(sounds(&app), 1);
        assert!(
            app.world()
                .resource::<crate::scripts::ObjStates>()
                .0
                .is_empty()
        );
        // Then the door, its trigger's state handed on as a `SetObjState`'s, and the queue goes.
        app.update();
        assert_eq!(
            app.world().resource::<crate::scripts::ObjStates>().0,
            [act_on("tor", true)]
        );
        app.update();
        assert!(app.world().get::<ScriptQueue>(smith).is_none());
    }

    #[test]
    fn a_queued_entry_done_with_more_waiting_ends_its_creatures_fight() {
        let mut tick = WorldTick::default();
        tick.gather(0.2);
        let mut app = App::new();
        app.add_plugins(bevy::prelude::MinimalPlugins)
            .insert_resource(tick)
            .insert_resource(Heights::house())
            .add_message::<sfx::Play>()
            .add_systems(Update, take_steps);
        let mut mind = ai::Mind::new(Vec3::ZERO, 0x1, (true, false), None, [70.0, 140.0], (76, 2));
        assert!(mind.sent_at());
        let steps = [Queued::Sound(7), Queued::Sound(8), Queued::Sound(9)];
        let guard = app
            .world_mut()
            .spawn((Transform::default(), mind, ScriptQueue(steps.into())))
            .id();
        let fights = |app: &App| app.world().get::<ai::Mind>(guard).unwrap().fighting();
        // A sound waits for no fight; done with two more waiting, the fight ends.
        app.update();
        assert!(!fights(&app));
        // The last entry done leaves the state as it is.
        app.update();
        app.world_mut()
            .get_mut::<ai::Mind>(guard)
            .unwrap()
            .sent_at();
        app.update();
        assert!(fights(&app));
        assert!(app.world().get::<ScriptQueue>(guard).is_none());
    }

    /// A creature waiting on a `Wait` while another lives (#3253): it leaves its fight and is
    /// deaf to its foes until the wait ends.
    #[test]
    fn a_creature_waiting_in_its_queue_is_deaf_until_its_wait_ends() {
        let mut tick = WorldTick::default();
        tick.gather(0.2);
        let mut app = App::new();
        app.add_plugins(bevy::prelude::MinimalPlugins)
            .insert_resource(tick)
            .insert_resource(Heights::house())
            .add_message::<sfx::Play>()
            .add_systems(Update, take_steps)
            .add_observer(hear_again);
        let mind = || {
            ai::Mind::new(
                Vec3::ZERO,
                0x1,
                (false, false),
                None,
                [70.0, 140.0],
                (76, 2),
            )
        };
        let mona = app
            .world_mut()
            .spawn((Transform::default(), ScriptName("mona".into()), mind()))
            .id();
        let mut fighter = mind();
        assert!(fighter.send_at_hero());
        let wait = Queued::Wait {
            other: Some("mona".into()),
            until: Until::Lives,
        };
        let waiter = app
            .world_mut()
            .spawn((Transform::default(), fighter, ScriptQueue([wait].into())))
            .id();
        app.update();
        let thinks = |app: &App| app.world().get::<ai::Mind>(waiter).unwrap().thinks.clone();
        assert!(thinks(&app).deaf() && !thinks(&app).fighting());
        // Mona gone, the wait ends, and it hears its foes again.
        app.world_mut().despawn(mona);
        app.update();
        assert!(!thinks(&app).deaf());
        assert!(app.world().get::<ScriptQueue>(waiter).is_none());
    }

    #[test]
    fn a_clip_outside_the_mode_moves_shaddar_alone() {
        assert!(clip_outside_the_mode(SHADDAR));
        assert!(!clip_outside_the_mode(76));
        assert!(!clip_outside_the_mode(183));
    }

    /// Anducar dead, his queue of `QIS_OnEnter56` (#2477): the dead one's walk goes nowhere, its
    /// `PlayAnim` lifts it out of its death and plays, and its queue runs on.
    #[test]
    fn a_dead_creatures_queued_clip_lifts_it_and_its_queue_runs_on() {
        let mut tick = WorldTick::default();
        tick.gather(0.2);
        let mut app = App::new();
        app.add_plugins(bevy::prelude::MinimalPlugins)
            .insert_resource(tick)
            .insert_resource(Heights::house())
            .add_message::<sfx::Play>()
            .add_systems(Update, take_steps);
        let mind = ai::Mind::new(
            Vec3::ZERO,
            0x1,
            (false, false),
            None,
            [70.0, 140.0],
            (76, 2),
        );
        let stashed = Stashed {
            mind,
            way: crowd::InTheWay::of(0, 0, None),
            lain: 0.0,
        };
        let walk = Queued::Walk {
            to: Vec3::new(2000.0, 0.0, 0.0),
            onto: None,
            orders: 0,
            noted: Vec3::ZERO,
        };
        let steps = [walk, Queued::Clip { slot: 144 }, Queued::Sound(7)];
        let anducar = app
            .world_mut()
            .spawn((
                Transform::default(),
                ScriptQueue(steps.into()),
                Dies::new(0),
                stashed,
            ))
            .id();
        // Its walk is dropped where it lies.
        app.update();
        assert!(app.world().get::<Walk>(anducar).is_none());
        assert!(app.world().get::<Dies>(anducar).is_some());
        // Its clip lifts it.
        app.update();
        assert!(app.world().get::<Dies>(anducar).is_none());
        assert!(app.world().get::<Stashed>(anducar).is_none());
        assert!(app.world().get::<ai::Mind>(anducar).is_some());
        assert!(app.world().get::<crowd::InTheWay>(anducar).is_some());
        assert!(app.world().get::<ScriptClip>(anducar).is_some());
        // The rest runs on as a living one's does.
        app.update();
        assert!(app.world().get::<ScriptQueue>(anducar).is_none());
        // One that kept nothing to stand with stays dead, its queue running on.
        let body = app
            .world_mut()
            .spawn((
                Transform::default(),
                ScriptQueue([Queued::Clip { slot: 144 }, Queued::Sound(7)].into()),
                Dies::new(0),
            ))
            .id();
        app.update();
        assert!(app.world().get::<Dies>(body).is_some());
        assert!(app.world().get::<ScriptClip>(body).is_none());
        app.update();
        assert!(app.world().get::<ScriptQueue>(body).is_none());
    }

    /// `refresh_a`'s `SetHP` 100 on Anducar dead (#2477): a share above 0 lifts a dead creature
    /// of the name at that health, one below 2 leaves it dead, and 0 leaves it as it is; one lifted
    /// dies again at a later 0 of the same run.
    #[test]
    fn a_set_hp_above_0_raises_a_dead_creature_of_its_name() {
        let mut app = App::new();
        app.add_plugins(bevy::prelude::MinimalPlugins)
            .init_resource::<crate::scripts::Healths>()
            .add_systems(Update, script_healths);
        let mind = ai::Mind::new(
            Vec3::ZERO,
            0x1,
            (false, false),
            None,
            [70.0, 140.0],
            (76, 2),
        );
        let dead = |app: &mut App, name: &str, most: u32| {
            app.world_mut()
                .spawn((
                    ScriptName(name.into()),
                    fight::Health {
                        current: 0,
                        most,
                        level: 40,
                    },
                    Dies::new(0),
                    Stashed {
                        mind: mind.clone(),
                        way: crowd::InTheWay::of(0, 0, None),
                        lain: 0.0,
                    },
                ))
                .id()
        };
        let anducar = dead(&mut app, "res:17810", 900);
        let faint = dead(&mut app, "faint", 100);
        let kept = dead(&mut app, "kept", 100);
        let set = |name: &str, share| crate::scripts::SetHealth {
            name: Some(name.into()),
            share,
        };
        let vilya = dead(&mut app, "res:17790", 3524);
        app.world_mut().resource_mut::<crate::scripts::Healths>().0 = vec![
            set("res:17810", 100),
            set("faint", 1),
            set("kept", 0),
            set("res:17790", 1),
            set("res:17790", 0),
        ];
        app.update();
        let health = |app: &App, entity| app.world().get::<fight::Health>(entity).unwrap().current;
        assert_eq!(health(&app, anducar), 900);
        assert!(app.world().get::<Dies>(anducar).is_none());
        assert!(app.world().get::<ai::Mind>(anducar).is_some());
        assert_eq!(health(&app, faint), 1);
        assert!(app.world().get::<Dies>(faint).is_some());
        assert_eq!(health(&app, kept), 0);
        assert!(app.world().get::<Dies>(kept).is_some());
        // Lifted and laid down again by one run, as `the_end_of_uw` does to Vilya.
        assert_eq!(health(&app, vilya), 0);
        assert!(app.world().get::<Dies>(vilya).is_some());
        assert!(app.world().get::<ai::Mind>(vilya).is_some());
    }

    #[test]
    fn a_queued_walk_runs_at_its_first_order_and_one_of_the_tag_0x66_walks() {
        let mut tick = WorldTick::default();
        tick.gather(0.2);
        let mut app = App::new();
        app.add_plugins(bevy::prelude::MinimalPlugins)
            .insert_resource(tick)
            .insert_resource(Heights::house())
            .add_message::<sfx::Play>()
            .add_systems(Update, take_steps);
        let walker = |app: &mut App, orders| {
            let mind = ai::Mind::new(Vec3::ZERO, 0x1, (true, false), None, [70.0, 140.0], (76, 2));
            let walk = Queued::Walk {
                to: Vec3::new(2000.0, 0.0, 0.0),
                onto: None,
                orders,
                noted: Vec3::ZERO,
            };
            app.world_mut()
                .spawn((Transform::default(), mind, ScriptQueue([walk].into())))
                .id()
        };
        let runs = walker(&mut app, 0);
        // `NPC_Goto`'s of the tag `0x66` counts its orders from 1 (#1892).
        let strolls = walker(&mut app, 1);
        app.update();
        let walk = |entity| app.world().get::<Walk>(entity).unwrap();
        assert!(walk(runs).runs());
        assert!(!walk(strolls).runs());
    }

    #[test]
    fn the_cinema_stops_the_hero_at_each_tick_while_its_queue_is_empty() {
        let mut tick = WorldTick::default();
        tick.gather(0.2);
        let mut app = App::new();
        app.add_plugins(bevy::prelude::MinimalPlugins)
            .insert_resource(tick)
            .insert_resource(crate::scripts::Mode {
                anim: true,
                cinema: true,
            })
            .add_systems(Update, cinema_holds);
        let sent = || {
            let mut body = hero::Body::standing();
            body.send(Vec3::new(500.0, 0.0, 0.0));
            body
        };
        let hero = app
            .world_mut()
            .spawn((sent(), crate::attack::Attack::ordered(Entity::PLACEHOLDER)))
            .id();
        // One that takes its queue's steps keeps them.
        let queued = app
            .world_mut()
            .spawn((sent(), ScriptQueue([Queued::Sound(7)].into())))
            .id();
        app.update();
        assert!(!app.world().get::<hero::Body>(hero).unwrap().ordered());
        assert!(app.world().get::<crate::attack::Attack>(hero).is_none());
        assert!(app.world().get::<hero::Body>(queued).unwrap().ordered());
        // Outside the cinema it goes where it is sent.
        app.world_mut()
            .resource_mut::<crate::scripts::Mode>()
            .cinema = false;
        *app.world_mut().get_mut::<hero::Body>(hero).unwrap() = sent();
        app.update();
        assert!(app.world().get::<hero::Body>(hero).unwrap().ordered());
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
    fn a_disarmed_creature_throws_down_its_right_hands_weapon_first() {
        let held = |item, hand| {
            (
                InHand {
                    item,
                    category: WEAPON,
                    kind: Some(1),
                    hand,
                },
                None,
            )
        };
        let arms = || Arms {
            base: fight::Base::default(),
            plain: false,
            model: String::new(),
            holding: vec![held(3001, Hand::Left), held(3002, Hand::Right)],
            worn: Vec::new(),
            mounted: false,
        };
        let mut world = bevy::prelude::World::new();
        let living = world.spawn((arms(), GlobalTransform::default())).id();
        let dying = world
            .spawn((arms(), GlobalTransform::default(), Dies::new(0)))
            .id();
        let follower = world.spawn((arms(), GlobalTransform::default())).id();
        crate::followers::rehire(&mut world, follower, living, (false, true));
        // One that the scripts keep to rise again keeps its arms (`+0x2B7` bit 8, #3088).
        let rises = world
            .spawn((arms(), GlobalTransform::default(), RisesAgain))
            .id();
        // The drops' tables are not read here: the weapons are not thrown, the hands empty all
        // the same.
        disarm(&mut world, living);
        disarm(&mut world, dying);
        disarm(&mut world, follower);
        disarm(&mut world, rises);
        assert!(world.get::<Rearms>(rises).is_none());
        assert_eq!(world.get::<Arms>(rises).unwrap().holding.len(), 2);
        let rearms = world.get::<Rearms>(living).unwrap();
        assert!(rearms.disarmed && rearms.away == [Hand::Right] && rearms.takes.is_empty());
        let kept: Vec<u32> = world
            .get::<Arms>(dying)
            .unwrap()
            .holding
            .iter()
            .map(|(held, _)| held.item)
            .collect();
        assert_eq!(kept, [3001]);
        assert!(world.get::<Rearms>(dying).is_none() && world.get::<Rearms>(follower).is_none());
    }

    #[test]
    fn a_skeleton_warrior_lets_its_lost_arms_item_go_and_keeps_the_others() {
        let held = |item, hand| {
            (
                InHand {
                    item,
                    category: WEAPON,
                    kind: Some(1),
                    hand,
                },
                None,
            )
        };
        let mut world = bevy::prelude::World::new();
        let warrior = world
            .spawn((
                Arms {
                    base: fight::Base::default(),
                    plain: false,
                    model: String::new(),
                    holding: vec![held(3001, Hand::Left), held(3002, Hand::Right)],
                    worn: Vec::new(),
                    mounted: false,
                },
                GlobalTransform::default(),
                Dies::new(0),
            ))
            .id();
        // Its right arm lost: the right hand's item goes, the left's stays (the drops' tables
        // are not read here, nothing is thrown).
        throw_hand(&mut world, warrior, 13);
        let kept = KeptItems::of(&mut world, warrior);
        assert_eq!(
            kept.held.iter().map(|&(item, ..)| item).collect::<Vec<_>>(),
            [3001]
        );
        // A hand of no slot of a hand throws nothing.
        throw_hand(&mut world, warrior, 1);
        assert_eq!(world.get::<Arms>(warrior).unwrap().holding.len(), 1);
        // The creature made in its place takes what it kept into its hands.
        let made = world.spawn_empty().id();
        kept.give(&mut world, made);
        let rearms = world.get::<Rearms>(made).unwrap();
        assert_eq!(rearms.takes, [(3001, Hand::Left, None)]);
        assert_eq!(rearms.away, [Hand::Left, Hand::Right]);
        // Kept nothing, the one made holds and wears nothing of its type's.
        KeptItems::default().give(&mut world, warrior);
        let rearms = world.get::<Rearms>(warrior).unwrap();
        assert!(rearms.takes.is_empty() && rearms.wears.is_empty());
        assert!(world.get::<Arms>(warrior).unwrap().holding.is_empty());
    }

    #[test]
    fn an_undead_made_again_wears_what_it_wore_and_none_of_its_types() {
        let piece = |item| Piece {
            item,
            model: format!("models/{item}.glb"),
            texture: None,
            hand: Hand::Right,
            mount: Mount::Worn,
        };
        let values = fight::ItemValues::default();
        let arms = |worn: Vec<(u32, u8, Option<fight::ItemValues>)>| Arms {
            base: fight::Base::default(),
            plain: false,
            model: String::new(),
            holding: Vec::new(),
            worn,
            mounted: false,
        };
        let mut world = bevy::prelude::World::new();
        let old = world
            .spawn(arms(vec![(5001, 1, Some(values)), (5002, 0, None)]))
            .id();
        let model = world.spawn((WornBy(old), piece(5001))).id();
        // A worn model of another's and one of a hand's are not its.
        let other = world.spawn_empty().id();
        world.spawn((WornBy(other), piece(5002)));
        world.spawn((
            WornBy(old),
            Piece {
                mount: Mount::held(WEAPON),
                ..piece(5002)
            },
        ));
        let kept = KeptItems::of(&mut world, old);
        assert_eq!(
            kept.worn,
            [
                (5001, 1, Some(Held::of_piece(&piece(5001))), Some(values)),
                (5002, 0, None, None)
            ]
        );
        assert!(kept.held.is_empty());
        // The one made in its place wore its type's own helmet, whose model goes.
        let made = world.spawn(arms(vec![(6001, 0, None)])).id();
        let its_own = world.spawn((WornBy(made), piece(6001))).id();
        kept.give(&mut world, made);
        assert!(world.get_entity(its_own).is_err());
        assert!(world.get_entity(model).is_ok());
        assert!(world.get::<Arms>(made).unwrap().worn.is_empty());
        let rearms = world.get::<Rearms>(made).unwrap();
        assert_eq!(rearms.wears, kept.worn);
    }

    #[test]
    fn a_walk_shows_its_clip_once_it_has_stepped() {
        let node = AnimationNodeIndex::new;
        let clips = Clips {
            graph: Handle::default(),
            idle: node(0),
            walk: Some(node(1)),
            run: Some(node(2)),
            fly: Some(node(3)),
            stance: Some(node(4)),
            idle_again: node(5),
            stance_again: Some(node(6)),
            talk: None,
            fun: [None; 4],
            attacks: Vec::new(),
            dying: Vec::new(),
            scripted: HashMap::new(),
            layered: None,
        };
        // A shot's end starts a standing clip anew in its other node, the same clip.
        assert_eq!(clips.again(node(0)), Some(node(5)));
        assert_eq!(clips.again(node(5)), Some(node(0)));
        assert_eq!(clips.again(node(4)), Some(node(6)));
        assert_eq!(clips.again(node(6)), Some(node(4)));
        assert_eq!(clips.again(node(1)), None);
        assert!(clips.same(node(6), node(4)) && clips.same(node(4), node(4)));
        assert!(!clips.same(node(1), node(4)) && !clips.same(node(5), node(4)));
        let mut walk = Walk::to(Vec3::ZERO, None, 70.0);
        // Not yet stepped, as a walk whose first step is blocked: it stands in its idle.
        assert_eq!(clips.moving(&walk), None);
        walk.moved = true;
        assert_eq!(clips.moving(&walk), Some(node(1)));
        let mut run = Walk::run(Vec3::ZERO, None, 140.0);
        run.moved = true;
        assert_eq!(clips.moving(&run), Some(node(2)));
        // A flyer aloft plays its flight's clip, the slot 179 (#2250).
        let mut flight = Walk::fly(Vec3::ZERO, None, 200.0);
        flight.moved = true;
        assert_eq!(clips.moving(&flight), Some(node(3)));
    }

    /// Where an NPC standing at `from` on `level` of the tests' house (`Heights::house`) stands
    /// once its walk `order` ends, run by [`walk`] a frame of 16 ms at a time, on which level, and
    /// the most it turned in one frame, in radians; `None` while it walks on after 300 frames.
    fn walked_in_the_house(from: Vec3, level: Level, order: Walk) -> Option<(Vec3, Level, f32)> {
        let heights = Heights::house();
        let layered = buildings::Layered::at(&heights, from.x, from.z, level);
        let mut app = App::new();
        app.add_plugins(bevy::prelude::MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_millis(16),
            ))
            .insert_resource(heights)
            .init_resource::<crowd::Crowd>()
            .init_resource::<hero::GameSpeed>()
            .init_resource::<WorldTick>()
            .add_systems(
                Update,
                (
                    |time: Res<Time>, mut tick: ResMut<WorldTick>| tick.gather(time.delta_secs()),
                    walk,
                )
                    .chain(),
            );
        let npc = app
            .world_mut()
            .spawn((
                Transform::from_translation(from),
                order,
                layered,
                crowd::InTheWay::of(0, 0, level),
                Npc {
                    gltf: Handle::default(),
                    model: String::new(),
                    names: ClipNames::default(),
                    graph: None,
                    player: None,
                    hands_empty: true,
                    instrument: None,
                    hits: Hits {
                        class: 0,
                        share: 1.0,
                    },
                    pace: 1.0,
                },
            ))
            .id();
        let mut turned = 0.0_f32;
        let mut facing = None;
        for _ in 0..300 {
            app.update();
            let world = app.world();
            let at = world.get::<Transform>(npc)?;
            if let Some(before) = facing.replace(at.rotation) {
                turned = turned.max(at.rotation.angle_between(before));
            }
            if world.get::<Walk>(npc).is_none() {
                return Some((
                    at.translation,
                    world.get::<crowd::InTheWay>(npc)?.level,
                    turned,
                ));
            }
        }
        None
    }

    #[test]
    fn a_walk_onto_the_ground_of_an_entrance_ends_inside() {
        // The chapel's door as the house has it: the door (5, 6) an entrance on the ground, the
        // cells in front of it on row 7 exits of the inside. The novice, on the ground three cells
        // out, is sent onto the door's ground, as her queue's walk is onto her own level (#1438).
        let house = Some((0, 0, 1));
        let point = |cell| {
            let point = paths::centre(cell);
            Vec3::new(point.x, 0.0, point.y)
        };
        let (door, out) = (point((5, 6)), point((5, 9)));
        let (at, level, turned) =
            walked_in_the_house(out, None, Walk::run(door, None, 276.0).onto(None))
                .expect("the walk ends");
        // It stands at the door inside, where the entrance took it.
        assert!((at - door).length() <= hero::ARRIVED, "{at}");
        assert_eq!(level, house);
        assert!(turned < 0.1, "{turned}");
        // Its queue, finding it on another level than the walk's, orders the walk again from
        // there: it had gone out through the exit before the door to come in onto its ground
        // again, round at every frame, its walk never ending; now the walk ends where it stands.
        let (again, level, turned) =
            walked_in_the_house(at, house, Walk::run(door, house, 276.0).onto(None))
                .expect("the walk ends");
        assert!((again - at).length() < 1.0, "{again}");
        assert_eq!(level, house);
        assert!(turned < 0.1, "{turned}");
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
        assert_eq!(to_melee.away, [Hand::Left, Hand::Right]);
        assert_eq!(to_melee.takes, [(1753, Hand::Right, None)]);
        let bow = Rearms::bow(1732);
        assert_eq!(bow.away, [Hand::Right, Hand::Left]);
        assert_eq!(bow.takes, [(1732, Hand::Left, None)]);
        // A hireling's sword for its bow: the bow off the left hand, the sword into the right.
        let values = fight::ItemValues::default();
        let handing = Rearms::handing(&[12], Some((1753, 13, values)));
        assert_eq!(handing.away, [Hand::Left, Hand::Right]);
        assert_eq!(handing.takes, [(1753, Hand::Right, Some(values))]);
        let sword = InHand::of([(1753, WEAPON, Some(1))]);
        assert_eq!(class_of(&sword), 1);
        assert!(!ranged_of(&sword));
    }

    #[test]
    fn a_search_sets_the_standing_clip_in_play_to_its_pace_once_its_blend_is_over() {
        use bevy::time::TimeUpdateStrategy;
        let mut app = App::new();
        app.add_plugins((
            bevy::prelude::MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            bevy::animation::AnimationPlugin,
        ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            50,
        )))
        .insert_resource(hero::GameSpeed(1.25))
        .add_systems(Update, paced);
        let mut stance_clip = AnimationClip::default();
        stance_clip.set_duration(0.82);
        let clip = app
            .world_mut()
            .resource_mut::<Assets<AnimationClip>>()
            .add(stance_clip);
        let mut graph = AnimationGraph::new();
        let root = graph.root;
        let [idle, stance, idle_again, stance_again] =
            [(); 4].map(|()| graph.add_clip(clip.clone(), 1.0, root));
        let graph = app
            .world_mut()
            .resource_mut::<Assets<AnimationGraph>>()
            .add(graph);
        // The shot's end starts the stance anew at the rate 1.0.
        let mut player = AnimationPlayer::default();
        let mut transitions = AnimationTransitions::new();
        transitions
            .play(&mut player, stance, Duration::ZERO)
            .repeat()
            .set_speed(1.25);
        let player = app
            .world_mut()
            .spawn((player, transitions, AnimationGraphHandle(graph.clone())))
            .id();
        let clips = Clips {
            graph,
            idle,
            walk: None,
            run: None,
            fly: None,
            stance: Some(stance),
            idle_again,
            stance_again: Some(stance_again),
            talk: None,
            fun: [None; 4],
            attacks: Vec::new(),
            dying: Vec::new(),
            scripted: HashMap::new(),
            layered: None,
        };
        let npc = app
            .world_mut()
            .spawn(Npc {
                gltf: Handle::default(),
                model: String::new(),
                names: ClipNames::default(),
                graph: Some(clips),
                player: Some(player),
                hands_empty: true,
                instrument: None,
                hits: Hits {
                    class: 0,
                    share: 1.0,
                },
                pace: 1.3,
            })
            .id();
        let speed = |app: &App| {
            app.world()
                .get::<AnimationPlayer>(player)
                .and_then(|player| player.animation(stance))
                .map(|active| (active.speed(), active.seek_time()))
        };
        // A search 0.1 s after its start, in its blend: the stance plays on at 1.0.
        for _ in 0..3 {
            app.update();
        }
        app.world_mut().entity_mut(npc).insert(SearchesWay);
        app.update();
        assert_eq!(speed(&app).map(|(speed, _)| speed), Some(1.25));
        assert!(app.world().get::<SearchesWay>(npc).is_none());
        // One 0.3 s after it: the pace's rate from its place on.
        for _ in 0..2 {
            app.update();
        }
        let before = speed(&app).map_or(0.0, |(_, place)| place);
        app.world_mut().entity_mut(npc).insert(SearchesWay);
        app.update();
        let (paced, place) = speed(&app).unwrap_or_default();
        assert!((paced - 1.625).abs() < 1e-5, "{paced}");
        assert!(place > before && place - before < 0.1, "{before} {place}");
    }

    #[test]
    fn a_graph_made_anew_starts_without_the_old_ones_clips() {
        // A thief's graph with its bow loops its stance at the node 4; with a sword it has no
        // stance, and the node 4 is its dying clip (#1538).
        let (idle, node_4) = (AnimationNodeIndex::new(1), AnimationNodeIndex::new(4));
        let mut player = AnimationPlayer::default();
        player.start(idle).repeat();
        player.start(node_4).repeat();
        let mut transitions = first_clip(&mut player, idle, 1.0);
        assert!(player.animation(node_4).is_none());
        let main = player.animation(idle).map(ActiveAnimation::repeat_mode);
        assert_eq!(main, Some(RepeatAnimation::Forever));
        // Its death plays the dying clip once.
        let dying = transitions.play(&mut player, node_4, BLEND);
        assert_eq!(dying.repeat_mode(), RepeatAnimation::Never);
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
        assert_eq!(position(ops, &named), Some(((3370, 2524, 0), 0)));
        let targets = goto_targets(&records);
        assert_eq!(targets["res:17095"], (3237, 2514, 0));
        // A record of two names sends the creature of its last (`0x49E210`, #3240).
        let twice = Record {
            kind: NPC_GOTO,
            body: Body::Operands(vec![
                Operand {
                    tag: 0x01,
                    value: Value::Str("HERO".into()),
                },
                Operand {
                    tag: 0x01,
                    value: Value::Str("Anducar".into()),
                },
                Operand {
                    tag: 0x04,
                    value: Value::I32x3([10, 20, 0]),
                },
            ]),
        };
        let targets = goto_targets(&[&twice]);
        assert_eq!(
            (targets.get("anducar"), targets.get("hero")),
            (Some(&(10, 20, 0)), None)
        );
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
    fn the_starts_effects_of_passing_types_far_from_it_end_at_once() {
        // The add-on's start in 97,60, the temple door's spikes at the cell 695,2630 (10,41).
        let start = (97, 60);
        assert!(ends_at_start(start, (695, 2630), Some(1174)));
        // Within the 3 × 3 about the start, of a persistent type, or no effect at all: kept.
        assert!(!ends_at_start(
            start,
            (98 * 64 + 5, 61 * 64 + 5),
            Some(1174)
        ));
        assert!(!ends_at_start(start, (96 * 64, 59 * 64), Some(1174)));
        assert!(!ends_at_start(start, (695, 2630), Some(1175)));
        assert!(!ends_at_start(start, (695, 2630), None));
        // Two sectors off along either axis is outside.
        assert!(ends_at_start(start, (99 * 64, 60 * 64), Some(891)));
        assert!(ends_at_start(start, (97 * 64, 62 * 64), Some(891)));
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
            parent: 0,
            model: Some(model.to_owned()),
            texture: None,
            counts: false,
            sound: 0,
            size: 50.0,
            group: 0,
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
                socketed: Vec::new(),
                name: 0,
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
            parent: 0,
            model: Some(model.to_owned()),
            texture: None,
            counts: false,
            sound: 0,
            size: 50.0,
            group: 0,
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

    /// `wea.bin`'s lists replace the scripts' pools, each the whole of its pool, an empty one
    /// emptying it; a pool past them stays the scripts' (#2651).
    #[test]
    fn the_weapon_groups_replace_the_scripts_pools() {
        let scripts = HashMap::from([
            (0, vec![10, 11]),
            (1, vec![20]),
            (2, vec![30]),
            (200, vec![1773]),
        ]);
        assert_eq!(with_groups(scripts.clone(), None), scripts);
        let mut groups = vec![Vec::new(); 3];
        groups[0] = vec![10, 11];
        groups[2] = vec![31, 31];
        let pools = with_groups(scripts.clone(), Some(&groups));
        assert_eq!(
            pools,
            HashMap::from([(0, vec![10, 11]), (2, vec![31, 31]), (200, vec![1773])])
        );
        // The installation's: pool 200 of the Schädelreißer's first item, 1833 for the scripts'
        // 1773, and the 256 lists.
        groups.resize(256, Vec::new());
        groups[200] = vec![1833];
        assert_eq!(with_groups(scripts, Some(&groups))[&200], [1833]);
    }

    #[test]
    fn a_create_npc_of_the_operand_54_is_armed_by_the_rules_and_by_its_hands() {
        // `Kopfgeldauswahl`'s targets carry `54` 0 (#3072); a negative one, or none, arms by no
        // rule, the last `54` counting.
        let op = |value| Operand {
            tag: BY_RULES,
            value: Value::I32(value),
        };
        assert!(by_rules(&[op(0)]));
        assert!(by_rules(&[op(-1), op(3)]));
        assert!(!by_rules(&[op(-1)]));
        assert!(!by_rules(&[op(0), op(-1)]));
        assert!(!by_rules(&[]));
        // Armed by its type's rule: a sword in its right hand, its clips by it.
        let (makers, _) = soldiers();
        let item = |slot, category: u8| Item {
            slot,
            kind: 2,
            class: 0,
            subclass: 0,
            name: "sword.grn".into(),
            texture: 0,
            sprite: 0,
            byte_03: 0,
            words_08: [0; 2],
            words_14: [0, 0, 0, 0, 0, 0, u32::from(category) << 16, 0],
            bytes_34: [0; 3],
            byte_57: 0,
            words_58: [0; 10],
        };
        let models = HashMap::from([("SWORD.GRN".to_owned(), "models/sword.glb".to_owned())]);
        let sword = item(1729, WEAPON);
        let gear = gear_table(
            &HashMap::from([(1729, &sword)]),
            &[],
            &HashMap::from([(1729, 1)]),
            (&models, &HashMap::new()),
            [1729].iter(),
        );
        let pools = HashMap::from([(5, vec![1729])]);
        let rules = [Rule {
            types: vec![313],
            items: [(5, 100), (0, 0), (-1, 0)],
            spells: Vec::new(),
            name: None,
        }];
        let mut placement = Placement::default();
        let held = arm_by_rules(
            &mut placement,
            (&rules, 313),
            (&pools, &gear, &makers.switches, &HashMap::new()),
            (
                false,
                |_, _: &mut spawning::Rng| None,
                |_: &mut spawning::Rng| None,
            ),
            &mut spawning::Rng::of(0, 0, 0),
        );
        assert_eq!(held, [1729]);
        assert_eq!(placement.holding.len(), 1);
        assert_eq!(placement.equipment[0].item, 1729);
        // A type of no rule, whose model has none of the general maker's slots and whose type
        // has no list of its own, takes nothing (#3185).
        let mut bare = Placement::default();
        let none = arm_by_rules(
            &mut bare,
            (&rules, 43),
            (&pools, &gear, &makers.switches, &HashMap::new()),
            (
                false,
                |_, _: &mut spawning::Rng| None,
                |_: &mut spawning::Rng| None,
            ),
            &mut spawning::Rng::of(0, 0, 0),
        );
        assert!(none.is_empty() && bare.equipment.is_empty());
        // A type of no rule whose model strikes two weapons (`0x77`) takes them by the general
        // maker at an odd draw: a weapon of the kind 1 asked; at an even one nothing.
        let slots = HashMap::from([(
            "bandit.glb".to_owned(),
            motions::Slots::of([(spawning::GENERAL_TWO_WEAPONS, "attack")]),
        )]);
        let (mut armed, mut unarmed, mut asked) = (0, 0, Vec::new());
        for seed in 0..40 {
            let mut general = Placement {
                model: "bandit.glb".to_owned(),
                ..Placement::default()
            };
            let held = arm_by_rules(
                &mut general,
                (&rules, 43),
                (&pools, &gear, &makers.switches, &slots),
                (
                    true,
                    |kind, _: &mut spawning::Rng| {
                        asked.push(kind);
                        Some(1729)
                    },
                    |_: &mut spawning::Rng| None,
                ),
                &mut spawning::Rng::of(0, 0, seed),
            );
            if held.is_empty() {
                unarmed += 1;
            } else {
                armed += 1;
                assert_eq!(held[0], 1729);
            }
        }
        assert!(armed > 0 && unarmed > 0, "{armed} {unarmed}");
        assert!(asked.iter().all(|&kind| kind == spawning::BLADE));
    }

    #[test]
    fn a_filled_creature_is_called_by_the_last_named_rule_of_its_type() {
        // 10,49's ice elf champion 146 and its 140, each named by its rule's `1D` (#3071); a
        // `1E`'s key and a `16`'s names too, the last tag of a record its name.
        let op = |tag, value| Operand { tag, value };
        let group = |ops: Vec<Operand>| Record {
            kind: MAKE_GROUP,
            body: Body::Operands(ops),
        };
        let tree = Tree {
            start: Vec::new(),
            quest: Vec::new(),
            functions: vec![opensacred_data::netscript::Function {
                name: "Sector10049Enter".into(),
                quest: -1,
                region: 0,
                records: vec![
                    group(vec![op(0x02, Value::I32(146)), op(0x1D, Value::I32(17769))]),
                    group(vec![op(0x02, Value::I32(140)), op(0x1D, Value::I32(17768))]),
                    group(vec![op(0x02, Value::I32(140))]),
                    group(vec![
                        op(0x02, Value::I32(260)),
                        op(0x1D, Value::I32(17557)),
                        op(0x1E, Value::Str("R25_DRACHENGARDE_MAG".into())),
                    ]),
                    group(vec![
                        op(0x02, Value::I32(261)),
                        op(0x16, Value::Str("R25_DRACHENGARDE".into())),
                    ]),
                ],
            }],
            quests: Vec::new(),
            regions: Vec::new(),
        };
        let rules = rules(&tree);
        let rules = &rules[&(10, 49)];
        let hash = |key: &str| opensacred_data::text::sacred_hash(key.as_bytes());
        assert_eq!(called_of(rules, 146), Some(hash("17769")));
        // A later rule of no name leaves the earlier's.
        assert_eq!(called_of(rules, 140), Some(hash("17768")));
        assert_eq!(called_of(rules, 260), Some(hash("R25_DRACHENGARDE_MAG")));
        assert_eq!(called_of(rules, 261), Some(hash("R25_DRACHENGARDE")));
        assert_eq!(called_of(rules, 258), None);
        // A later named rule calls it by its own.
        let mut later = rules.clone();
        later.push(Rule {
            types: vec![146],
            items: [(-1, 0), (0, 0), (-1, 0)],
            spells: Vec::new(),
            name: Some(7),
        });
        assert_eq!(called_of(&later, 146), Some(7));
    }

    #[test]
    fn a_filled_magician_takes_one_of_its_rules_spells() {
        let rule = |types: Vec<u32>, spells: Vec<u8>| Rule {
            types,
            items: [(-1, 0), (0, 0), (-1, 0)],
            spells,
            name: None,
        };
        let rules = [rule(vec![258, 262], vec![5, 43]), rule(vec![258], vec![34])];
        let mut rng = spawning::Rng::of(48, 39, 0);
        let taken: HashSet<u8> = (0..50)
            .filter_map(|_| spell_of(&rules, 258, &mut rng))
            .collect();
        // By the first rule that names it, either of its spells.
        assert_eq!(taken, HashSet::from([5, 43]));
        assert_eq!(spell_of(&rules, 999, &mut rng), None);
        assert_eq!(
            spell_of(&[rule(vec![311], Vec::new())], 311, &mut rng),
            None
        );
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
        let rule = |types: Vec<u32>, items| Rule {
            types,
            items,
            spells: Vec::new(),
            name: None,
        };
        let rules = [
            rule(vec![306, 313], [(125, 100), (50, 100), (-1, 0)]),
            rule(vec![304], [(134, 100), (50, 100), (-1, 0)]),
            rule(vec![306], [(134, 100), (0, 0), (-1, 0)]),
            rule(vec![307], [(124, 100), (50, 100), (-1, 0)]),
        ];
        let gear = |category, kind| Gear {
            category,
            kind,
            parent: 0,
            model: Some("m.glb".into()),
            texture: None,
            counts: true,
            sound: 0,
            size: 50.0,
            group: 0,
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
