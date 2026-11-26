//! The saves (#1095; docs/re/saves.md): a game kept in a file of its own and given back, as the
//! original's `cEngine::save` keeps its game in `SAVE/GAMEnn.PAK`, in the game's own versioned
//! format (PLAN.md ADR-08). A save is postcard: its [`Header`] and its picture (`save_picture`,
//! #1991), which the saves' lists read alone, then its [`Hero`], its [`Scripts`] and its
//! [`World`]; a save of another version is refused, as the original's load refuses one
//! (`0x61B6F0`). The saves lie in `saves/` beside the settings' file.
//!
//! The first part (#1957) keeps the hero: its class, name, difficulty, values, items, arts,
//! combos, effects, statistics and place, and the calendar. A load starts a game of the save's
//! hero as a new game starts, then puts the saved values back once the hero stands, after the
//! resets of a game's start (`OnEnter(InGame)`), the hero where it stood.
//!
//! The second (#1958) keeps the scripts: the game's seed, their state, the functions done and
//! their `rand()`, and what their play changed of the placements. A load's start runs of the
//! save's seed, making the saved game's tree and placements again, and takes the save's scripts
//! in place of its run's, none of its windows shown again ([`crate::scripts::Started::resume`]),
//! and the play's records of the world placed again ([`crate::npcs::Npcs::resume`]).
//!
//! The third (#1959) keeps the world: the creatures that the sectors keep, those of the sectors in
//! the world gone as their sectors keep them going, which a load's sectors put back as they come
//! in ([`crate::npcs::Npcs::remember`]); the hero's followers, put back beside it; its own horse,
//! put back where it stood, with its bridle (#2301, #2316); the drops; the town stash; the
//! merchants' stock; the land explored and the map's pin; the weather; the buildings' states
//! (#2888); the scripts' traps as they stand, the fields of pills and the fences of lightning
//! (#3450); and the scripts' gold arrows (#3456).
//!
//! The quick save (#1988): F9 and Control+Z save the game as `QUICKSAVE` in the slot 0's file
//! `GAME00` where the interface allows a save ([`allowed`]), F8 and Control+X load it. The
//! scripts' autosave (#1989): an `AutoSave` saves the game into the first free slot, named by its
//! number's text, where `[gameplay] autosave` is on ([`autosave`]). Both take the save's picture
//! first and wait for it ([`waited`]).
//!
//! The saves' window (#1990, `saves_window`) lists the slots' files with their pictures
//! ([`slots`]) and erases one ([`erase`]). An exported hero's character is put on the hero of a
//! new game once it stands ([`Import`], #2552).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};
use bevy::ecs::entity::MapEntities;
use bevy::prelude::{
    App, Commands, Entity, IntoScheduleConfigs, NextState, OnEnter, Plugin, Projection, Query,
    Real, Res, ResMut, Resource, Time, Transform, Update, Vec2, Vec3, With, in_state, info, not,
    resource_exists, warn,
};
use opensacred_rules::arts::Arts;
use opensacred_rules::drops::Difficulty;
use opensacred_rules::explored::{Explored, Sectors};
use opensacred_rules::fight::Experience;
use opensacred_rules::trade::stock::Stock;
use serde::{Deserialize, Serialize};

use crate::art_slots::ArtSlots;
use crate::combos::HeroCombos;
use crate::daynight::Clock;
use crate::effects::Effects;
use crate::explored::{Land, MapSectors};
use crate::fight::Health;
use crate::frame::Step;
use crate::game::{AppState, Class, NewGame};
use crate::hero::{Arrival, Body};
use crate::merchant::Wares;
use crate::names::{Loading, Names, Refs, Saving};
use crate::pickup::{self, Pack};
use crate::save_picture::Picture;
use crate::scripts::{HeroHome, Teleport};
use crate::sets::WeaponSets;
use crate::settings::SettingsFile;
use crate::statistics::{History, Kills, Tally, Wins};
use crate::terrain::Heights;
use crate::town_stash::Chest;
use crate::world_map::Map;
use crate::{arts, drops, making};

/// The saves' version, raised at each change of a saved structure's layout, which postcard reads
/// by the fields' order alone, and which a test traces from the types and pins beside it
/// (`the_saves_layout_is_its_versions`, #2306): a save of another is refused. The version 5
/// (#1125) keeps the levels of the scripts' items lying and the creatures' placements as they
/// stand since #2249, #2290 and #2305, which the version 4 had changed under it (#2306); the
/// version 6 keeps no effect's source (#2308); the version 7 the hero's own horse (#2301); the
/// version 8 a stable's horse's trader (#2300); the version 9 a fill's rider on its horse (#2288);
/// the version 10 the hero's own horse's bridle (#2316); the version 11 the items' names that a
/// carried hero's file keeps (#2390); the version 12 a game's script tree (#2472); the version 13
/// a creature's defending slot, its parry's (#2487); the version 14 whether its model has a dying
/// clip in the slot 173 (#2514); the version 15 the hero's wins of each difficulty (#2464); the
/// version 16 the type of a placement's prison, a magic one's or a bubble's (#2750); the version
/// 17 the item on the cursor (#2822); the version 18 the runes of its class given the hero
/// (#2799); the version 19 whether a follower rises again and how long one has lain dead to
/// rise (#2947); the version 20 the creatures' queues and the scripts' modes (#2890); the version
/// 21 the scripts' functions done by their files and keys (#2810); the version 22 the difficulty
/// a made item was made on (#2807); the version 23 the stalls that purchases emptied (#2907); the
/// version 24 the hero's counts of its quests (#2886); the version 25 the buildings' states
/// (#2888); the version 26 the function that a script's plain item's taking runs (#2891); the
/// version 27 the type of a placement's trap of the scripts (#2916); the version 28 a
/// placement's vision's mark (#3064); the version 29 the name a fill's creature is called by
/// (#3071); the version 30 a placement's emerging (#3075); the version 31 the `Use:` function
/// that a drop keeps (#3099); the version 32 the script names of the items carried and of a drop
/// (#3147); the version 33 those names kept in the items themselves, the weapon sets' too
/// (#3217); the version 34 the art slots' second bank (#2928); the version 35 the lens's places
/// of `SetMapIcon` (#2932); the version 36 a placement's fire of list 4 (#3286); the version 37
/// a `ReplaceSpawn`'s flags word (#3073); the version 38 the mark of a bonus against the undead
/// on a creature (#3385); the version 39 the dying clips by their slots (#3400); the version 40
/// the scripts' traps as they stand (#3450); the version 41 the scripts' gold arrows (#3456); the
/// version 42 a follower's entry in the portraits' column (#3102); the version 43 the hero's
/// shots' chain (#3568); the version 44 the marks of the items that the scripts put into the
/// backpack (#3543); the version 45 the hero's knock-downs' marks (#3673); the version 46 whether a
/// creature's attack's clips are a stand-in (#3314); the version 47 the camera's zoom (#3740); the
/// version 48 the hero's and the kept creatures' headings (#3744); the version 49 the hero's walk
/// under way (#3742); the version 50 whether the hero rides its own horse (#3743); the version 51
/// the quest book and the inventory shown (#3741); the version 52 the names of the creatures that
/// the effects' sources are and the serials of the creatures made in play (#3755); the version 53
/// the part of what lives, the creatures' doings (#3756); the version 54 the creatures' minds
/// (#3757); the version 55 the respawn's gate's passes and clock (#3786); the version 56 the
/// creatures' walks and heights aloft (#3758); the version 57 the strikes, the scripts' clips and
/// the hero's attack mid-way (#3759); the version 58 the arrows and the bolts in flight (#3760); the
/// version 59 the sectors' `Enter` creatures as they stood (#3761); the version 60 the bodies
/// (#3762); the version 61 the hero's orders and the drops' names (#3763); the version 62 the hero's
/// arts and combo under way (#3764); the version 63 the special arts' states and the blasts
/// (#3765); the version 64 the creatures' casts and arts (#3766); the version 65 the spells'
/// states and the changelings' animals (#3767); the version 66 the special creatures' states
/// (#3768); the version 67 the things thrown and shot (#3769); the version 68 the spells'
/// objects (#3770); the version 69 the elves' and the Vampiress's (#3771); the version 70 the
/// Dwarf's and the Daemon's, and the breaths (#3772); the version 71 the looks' bursts (#3773);
/// the version 72 the bolts' and the guns' looks (#3774); the version 73 the Seraphim's and the
/// Mage's looks (#3775); the version 74 the elves' looks (#3776); the version 75 the Dwarf's,
/// the Daemon's and the Gladiator's looks (#3777); the version 76 the fight's and the bodies'
/// looks (#3778); the version 77 the bearers' looks and the casts' glows (#3779); the version
/// 78 the shields', the casts' sights and the scripts' objects' looks (#3814); the version 79
/// the Vampiress's and the souls' looks, the souls and the soulcatchers (#3809); the version 80
/// the marks', the blood's and the strikes' looks (#3812); the version 81 the teleports' sights'
/// places (#3822); the version 82 the health rings' timers (#3780).
pub const VERSION: u32 = 82;
/// The saves' directory beside the settings' file, and their extension.
const DIRECTORY: &str = "saves";
const EXTENSION: &str = "sav";

/// What the saves' list shows of a save, read alone: the version, its name, the hero's class,
/// name, difficulty and level, the game's calendar and the computer's time of the save, as the
/// original's header holds them (`0x619BF0`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Header {
    pub version: u32,
    pub name: String,
    pub class: Class,
    pub hero: String,
    pub difficulty: Difficulty,
    pub level: u32,
    /// The game's time of day, in seconds.
    pub clock: f64,
    /// The computer's time of the save, in seconds since 1970.
    pub saved: u64,
    /// The game's script tree where it is not its class's in Ancaria (`NewGame::tree`): an
    /// Underworld game's (`Class::underworld_tree`), which its load runs again (#2472).
    pub tree: Option<String>,
}

/// The hero as a save keeps it.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Hero {
    /// Where it stands, in world units over the ground's plane, and its floor, 0 the ground.
    pub at: (f32, f32),
    pub floor: u8,
    /// The heading it faces, in a placement's degrees (`npcs::degrees`), as the original's
    /// creature's record keeps its direction `+0x70` (`0x44FC50`, #3744).
    pub heading: f32,
    pub experience: Experience,
    pub base: opensacred_rules::fight::Base,
    pub health: u32,
    pub effects: Effects,
    pub arts: Arts,
    pub slots: ArtSlots,
    pub selected: Option<u16>,
    pub combos: HeroCombos,
    pub pack: pickup::Kept,
    pub sets: WeaponSets,
    pub home: Option<Teleport>,
    pub tally: Tally,
    pub kills: Kills,
    pub history: History,
    /// The hero's wins of each difficulty (#2464).
    pub wins: Wins,
    /// The runes of its class that the quests gave it (`rewards::RunesGiven`, #2799).
    pub runes_given: u32,
    /// Its counts of its quests' rewards and of those since its last set's piece
    /// (`rewards::QuestCounts`, #2886).
    pub quests: opensacred_rules::rewards::QuestCounts,
    /// The survival counter's seconds (`drops::Survival`), which the original's save keeps as
    /// whole seconds in its header's `+0x44` (#3785).
    pub survival: f32,
    pub clock: Clock,
    /// The steps of its queue under the scripts' anim mode, a `Teleport`'s among them (#2890).
    pub queue: crate::npcs::SavedQueue,
    /// Its shots' chain (`attack::ShotChain`), which the original's creature's stream keeps
    /// (`0x54B761`, #3568).
    pub chain: f32,
    /// Its knock-downs' marks (`fight::KnockMarks`), the stats' `+0x2B7` word that the stream
    /// keeps whole (`0x54B856`, #3673).
    pub marks: u8,
    /// The camera's zoom, which the original's save keeps as its current view's `+0xB60` in its
    /// views' part `0x82` and its load gives back (`0x6288F0`, #3740).
    pub zoom: f32,
    /// Where it was going, which it goes on to after the load (#3742).
    pub walk: Option<Walk>,
    /// The windows that showed, which show again after the load (#3741).
    pub windows: Windows,
}

/// Whether the quest book and the inventory's windows show, as the original's save keeps them in
/// its interface's part `0x9C` (the interface manager's `+0xA8` and `+0x84`, `0x75A970`,
/// `0x75A990`) and its load shows or hides each to match (#3741).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Windows {
    pub book: bool,
    pub inventory: bool,
}

/// Where the hero was going as a save keeps it (#3742): its target's point and, where a click
/// chose the level it lies on, that level's floor, 0 the ground; as the original's creature's
/// record keeps its moving action with its point and floor (`+0x150` to `+0x1A3`, `0x54B3D0`), so
/// that a load walks on to the same target. Its way is found anew from where it stands, as the
/// original's search for the player's way runs again every half a second (docs/re/movement.md,
/// "Searching again (`0x4F4770`)").
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Walk {
    pub to: (f32, f32, f32),
    pub floor: Option<u8>,
}

impl Walk {
    /// The hero's walk under way: its target and the level a click chose of it, `heights`
    /// telling the level's floor.
    fn of(target: Vec3, onto: Option<crate::terrain::Level>, heights: Option<&Heights>) -> Self {
        let floor = onto.map(|level| {
            heights
                .and_then(|heights| heights.floor(level))
                .map_or(0, |(_, floor)| floor)
        });
        Self {
            to: (target.x, target.y, target.z),
            floor,
        }
    }

    /// Its target and the level to send the hero onto there ([`Body::send_to`]): the floor's
    /// level of the building that holds the target's cell on it, the ground's for the floor 0, and
    /// whatever level the way reaches where no building's floor holds the cell.
    fn order(&self, heights: &Heights) -> (Vec3, Option<crate::terrain::Level>) {
        let to = Vec3::new(self.to.0, self.to.1, self.to.2);
        let cell = crate::paths::cell_of(to.x, to.z);
        let level = match self.floor {
            None => None,
            Some(0) => Some(None),
            Some(floor) => heights
                .building_on_floor(cell, usize::from(floor))
                .and_then(|building| heights.floor_level(building, usize::from(floor)))
                .map(Some),
        };
        (to, level)
    }
}

/// A loaded hero's walk to give it once it has landed where the save put it back (#3742).
#[derive(Resource)]
struct WalkOn(Walk);

/// A loaded hero's own horse that it rode at the save, which it mounts again once it has landed,
/// and its health then, which the horse under it raises the most of (`riding::Saddle`, #3743).
#[derive(Resource)]
struct RideOn {
    horse: Entity,
    health: u32,
}

/// The scripts as a save keeps them (#1958; the original's quests, `sdk_SAVELOAD_SERIAL`): the
/// game's seed (`seed::Game`), whose start makes the saved game's tree and placements again, the
/// scripts' state, functions done and `rand()`, and what their play changed of the placements.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
// Named apart from another saved type of its name, which the saves' layout test traces by
// name (#2306); postcard writes no names.
#[serde(rename = "SavedScripts")]
pub struct Scripts {
    pub seed: u64,
    pub scripts: crate::scripts::Saved,
    pub npcs: crate::npcs::Saved,
}

/// The world as a save keeps it (#1959; the original's object manager, its weather and its map,
/// docs/re/saves.md): the creatures that the sectors keep, those of the sectors in the world gone
/// as their sectors keep them going (`npcs::Creatures`), the hero's followers, the drops lying
/// and flying, the town stash, the merchants' stock, the land explored and the sectors entered,
/// the world map's pin, the weather, its timers of real time from the save's, the buildings'
/// states, the scripts' traps as they stand and their gold arrows.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct World {
    pub creatures: crate::npcs::Creatures,
    pub followers: Vec<crate::npcs::Following>,
    /// The hero's own horse (#2301), the item of its slot 0, its bridle (#2316), and whether the
    /// hero rides it, mounted again once the load's hero has landed (#3743).
    pub horse: Option<crate::npcs::Following>,
    pub bridle: Option<opensacred_rules::drops::Dropped>,
    pub ridden: bool,
    pub drops: Vec<drops::Laid>,
    pub stash: Chest,
    pub wares: Stock,
    pub land: Explored,
    pub sectors: Sectors,
    pub pin: Option<(Vec2, Vec2)>,
    pub weather: Option<opensacred_rules::weather::Weather>,
    /// The stables' stalls that purchases emptied, until their traders fill them (#2907).
    pub stalls: Vec<crate::stables::SavedStall>,
    /// The buildings' states, which the hero's steps and the scripts gave (#2888).
    pub buildings: crate::buildings::SavedBuildings,
    /// The scripts' fields of pills and ghosts laid, their pills eaten among them, and their
    /// fences of lightning, the makers' timers and the segments flying (#3450).
    pub fields: Vec<opensacred_rules::script_traps::Field>,
    pub fences: crate::laser_fences::Saved,
    /// The scripts' gold arrows standing, each one's place and colour (`beacons`, #3456).
    pub arrows: Vec<(Vec3, u32)>,
}

/// What lives in the world as a save keeps it (#3756, the epic #3753), read after the world: the
/// names that the save's references point at (`names::Refs`, #3755) and the creatures' doings
/// (`doings`), which a load gives back once its scene is whole ([`resume`]).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Live {
    pub refs: Refs,
    pub doings: crate::doings::Doings,
    pub things: Things,
    /// The dead creatures' bodies, lying and dying (#3762).
    pub bodies: Vec<crate::npcs::SavedBody>,
    /// The changelings' animals, which no sector keeps (#3767).
    pub animals: Vec<crate::npcs::SavedAnimal>,
    /// The scripts' special objects (#3768).
    pub specials: Specials,
    /// Whether the weapon that the hero hurls is out of his hand, flying or lying (#3769).
    pub hurled: bool,
    /// The looks (#3773).
    pub looks: Looks,
}

/// The looks as a save keeps them (#3773, the epic #3753): the bursts, of which the hits'
/// sparks, the changelings' sparks, the poison rings', the lightnings' and the scripts' smokes and
/// the level-ups' sparks are made (`puffs`).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Looks {
    pub puffs: Vec<crate::puffs::SavedPuff>,
    /// The bolts' and the guns' looks (#3774): the grenades' impacts, the hard rays' steps, the
    /// chain bolts and their impacts, and the trails that outlive their bolts (a flying bolt's
    /// goes with it, `bolts::SavedBolt`).
    pub impacts: Vec<crate::grenade_impacts::SavedImpact>,
    pub rays: Vec<crate::hard_rays::SavedRay>,
    pub chain_bolts: (
        Vec<crate::chain_bolts::SavedChainBolt>,
        Vec<crate::chain_bolts::SavedChainImpact>,
    ),
    pub trails: Vec<crate::bolt_trails::SavedTrail>,
    /// The guns' flashes, the beams' segments, the club arrows' rings and the arts' streaks.
    pub flashes: Vec<crate::gun_fires::SavedFlash>,
    pub beams: Vec<crate::beams::SavedSegment>,
    pub rings: Vec<crate::arrow_sights::SavedRing>,
    pub streaks: Vec<crate::weapon_streaks::SavedStreaks>,
    /// The Seraphim's and the Mage's looks (#3775): the explosions, the meteors, the light
    /// strikes, the Seraphim's sights, the gusts, the ice splinters that outlive their bolts and
    /// the horses' floops; the auras' own looks go with their auras (`spells::SavedSpellObject`).
    pub blasts: Vec<crate::explosions::SavedBlast>,
    pub meteors: Vec<crate::meteors::SavedMeteor>,
    pub light_strikes: Vec<crate::light_strikes::SavedLightStrike>,
    pub sera_sights: Vec<crate::sera_sights::SavedSight>,
    pub gusts: Vec<crate::twisters::SavedGust>,
    pub splinters: Vec<crate::ice_splinters::SavedSplinters>,
    pub floops: Vec<crate::floops::SavedFloops>,
    /// The Wood Elf's, the Dark Elf's and the Vampiress's looks (#3776): the multiple shots'
    /// sights and the round strikes' rings, the plant cages, the nature's healings, the dark wards
    /// and the dark helices; the spells' objects' own looks go with their objects
    /// (`spells::SpellLook`), and the spikes with their flames (`creature_casts::SavedCastObject`).
    pub elf_sights: (
        Vec<crate::elf_sights::SavedElfSight>,
        Vec<crate::elf_sights::SavedElfRing>,
    ),
    pub cages: Vec<crate::plant_cages::SavedCage>,
    pub healings: Vec<crate::plant_healings::SavedHealing>,
    pub wards: Vec<crate::dark_wards::SavedWard>,
    pub helices: Vec<crate::dark_helices::SavedHelix>,
    /// The Dwarf's, the Daemon's and the Gladiator's looks (#3777): the armours, the berserks, the
    /// burrowings, the greeds, the war cries' signs, the guns' sights, the forces of hell, the
    /// demons' deaths, the forms, the Gladiator's sight and the war cries; the spells' objects'
    /// own looks go with their objects (`spells::SpellLook`).
    pub armours: Vec<crate::dwarf_armours::SavedSight>,
    pub berserks: Vec<crate::dwarf_berserks::SavedSight>,
    pub burrows: Vec<crate::dwarf_burrows::SavedSight>,
    pub greeds: Vec<crate::dwarf_greeds::SavedSight>,
    pub cry_signs: Vec<crate::war_cry_signs::SavedSight>,
    pub gun_sights: Vec<crate::dwarf_fires::SavedSight>,
    pub forces: Vec<crate::hell_forces::SavedForce>,
    pub deaths: Vec<crate::demon_deaths::SavedDeath>,
    pub forms: Vec<crate::demon_forms::SavedForm>,
    pub glad_sight: Option<opensacred_rules::particles::GladSight>,
    pub cries: Vec<crate::war_cries::SavedCry>,
    /// The fight's and the bodies' looks (#3778): the knockouts, the doom signs, the teleports,
    /// the stones, the reanimations, the burning bones, the goblins' impacts, the banishing's runes
    /// and the battle fogs.
    pub knockouts: Vec<crate::knockouts::SavedLook>,
    pub doom_signs: Vec<crate::doom_signs::SavedLook>,
    pub teleports: Vec<crate::teleports::SavedLook>,
    pub stones: Vec<crate::stones::SavedLook>,
    pub reanimations: Vec<crate::reanimations::SavedLook>,
    pub burning_bones: Vec<crate::burning_bones::SavedLook>,
    pub goblin_impacts: Vec<crate::goblin_impacts::SavedLook>,
    pub runes: Vec<crate::banishing_runes::SavedLook>,
    pub battle_fogs: Vec<crate::battle_fogs::SavedLook>,
    /// The bearers' looks and the casts' glows (#3779): the healings', the speeds' and the
    /// holdings' sights, and the looks that outlive their objects of the traps' balls and
    /// landings, the homing fires and the flying skulls.
    pub healing_sights: Vec<crate::healings::SavedLook>,
    pub speed_sights: Vec<crate::speed_sights::SavedLook>,
    pub holding_sights: Vec<crate::holding_sights::SavedLook>,
    pub trap_balls: Vec<crate::trap_balls::SavedLook>,
    pub trap_landings: Vec<crate::trap_landings::SavedLook>,
    pub homing_fires: Vec<crate::homing_fires::SavedLook>,
    pub flying_skulls: Vec<crate::flying_skulls::SavedLook>,
    /// The shields', the casts' sights and the scripts' objects' looks (#3814): the shields and
    /// the regenerations, the casts' sights, the bubbles that outlive their bolts and their sonar
    /// rings, the bubble prisons, the magic prisons and the stargates.
    pub shields: Vec<crate::shields::SavedLook>,
    pub cast_sights: Vec<crate::cast_sights::SavedLook>,
    pub bubbles: Vec<crate::bubble_traps::SavedLook>,
    pub sonars: Vec<crate::bubble_traps::SavedSonar>,
    pub bubble_prisons: Vec<crate::bubble_prisons::SavedLook>,
    pub prisons: Vec<crate::prisons::SavedLook>,
    pub stargates: Vec<crate::stargates::SavedLook>,
    /// The Vampiress's and the souls' looks (#3809): the domination's rings and the raising's
    /// pentagrams, the bites' sights, her casts' sights, the souls' sparks that outlive their
    /// souls, and the soulcatchers with their objects.
    pub vamp_sights: Vec<crate::vamp_sights::SavedLook>,
    pub bite_sights: Vec<crate::bite_sights::SavedLook>,
    pub vamp_casts: crate::vamp_casts::SavedSights,
    pub soul_sparks: Vec<crate::soul_sparks::SavedLook>,
    pub soulcatchers: Vec<crate::soulcatchers::SavedCatcher>,
    /// The marks', the blood's and the strikes' looks (#3812): the marks, the blood fountains,
    /// the body parts, the blows' sights, the summonings' sights, the big demon's summonings,
    /// the lightnings, the poison rings and the hit sparks' lights.
    pub marks: Vec<crate::marks::SavedMark>,
    pub fountains: Vec<crate::blood_fountains::SavedFountain>,
    pub body_parts: Vec<crate::body_parts::SavedPart>,
    pub blows: Vec<crate::blow_sights::Kept>,
    pub summon_looks: Vec<crate::summon_looks::SavedLook>,
    pub summonings: Vec<crate::summonings::SavedLook>,
    pub lightnings: Vec<crate::lightning_strikes::SavedStrike>,
    pub poison_rings: Vec<crate::poison_rings::SavedRing>,
    pub spark_lamps: Vec<crate::hit_sparks::SavedLamp>,
}

impl MapEntities for Looks {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.puffs.map_entities(mapper);
        self.flashes.map_entities(mapper);
        self.streaks.map_entities(mapper);
        self.light_strikes.map_entities(mapper);
        self.sera_sights.map_entities(mapper);
        self.gusts.map_entities(mapper);
        self.floops.map_entities(mapper);
        self.elf_sights.0.map_entities(mapper);
        self.elf_sights.1.map_entities(mapper);
        self.cages.map_entities(mapper);
        self.healings.map_entities(mapper);
        self.wards.map_entities(mapper);
        self.helices.map_entities(mapper);
        self.armours.map_entities(mapper);
        self.berserks.map_entities(mapper);
        self.burrows.map_entities(mapper);
        self.greeds.map_entities(mapper);
        self.cry_signs.map_entities(mapper);
        self.gun_sights.map_entities(mapper);
        self.forces.map_entities(mapper);
        self.forms.map_entities(mapper);
        self.cries.map_entities(mapper);
        self.knockouts.map_entities(mapper);
        self.teleports.map_entities(mapper);
        self.stones.map_entities(mapper);
        self.burning_bones.map_entities(mapper);
        self.healing_sights.map_entities(mapper);
        self.speed_sights.map_entities(mapper);
        self.holding_sights.map_entities(mapper);
        self.shields.map_entities(mapper);
        self.cast_sights.map_entities(mapper);
        self.bubble_prisons.map_entities(mapper);
        self.prisons.map_entities(mapper);
        self.stargates.map_entities(mapper);
        self.vamp_sights.map_entities(mapper);
        self.bite_sights.map_entities(mapper);
        self.vamp_casts.map_entities(mapper);
        self.soulcatchers.map_entities(mapper);
        self.fountains.map_entities(mapper);
        self.blows.map_entities(mapper);
        self.summonings.map_entities(mapper);
        self.lightnings.map_entities(mapper);
    }
}

/// The scripts' special objects as a save keeps them (#3768, the epic #3753): Shaddar's slaves
/// and the big demon's illusion, which their sectors place no more and a load makes again of
/// their placements, and the zhadums' guards, whose sector's `Enter` makes them again.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Specials {
    pub slaves: Vec<crate::shaddar_slaves::SavedSlave>,
    pub illusions: Vec<crate::demon_illusions::SavedIllusion>,
    pub zhadums: Vec<crate::zhadums::SavedZhadum>,
}

impl MapEntities for Specials {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.slaves.map_entities(mapper);
    }
}

/// The things in flight as a save keeps them (#3760, the epic #3753): the arrows flying and stuck
/// (`arrows`) and the bolts (`bolts`), each with its references by their names.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Things {
    pub arrows: Vec<crate::arrows::SavedArrow>,
    pub stuck: Vec<crate::arrows::SavedStuck>,
    pub bolts: Vec<crate::bolts::SavedBolt>,
    /// The blasts spreading (#3765).
    pub blasts: Vec<crate::blast::SavedBlasting>,
    /// The things thrown and shot (#3769): the mortar's shells, the hero's hurled weapon, the
    /// spits, the creatures' daggers, the creatures' casts' objects, the summonings and the
    /// skulls.
    pub shells: Vec<crate::guns::Shell>,
    pub hurled: Vec<crate::hurled::SavedHurled>,
    pub spits: Vec<crate::spits::SavedSpit>,
    pub daggers: Vec<crate::creature_arts::SavedDagger>,
    pub casts: Vec<crate::creature_casts::SavedCastObject>,
    pub summonings: Vec<crate::followers::SavedSummoning>,
    pub skulls: Vec<crate::spells::SavedSkull>,
    /// The spells' objects (#3770).
    pub spells: Vec<crate::spells::SavedSpellObject>,
    /// The Daemon's breaths and the dragons' fires (#3772).
    pub breaths: crate::breath::SavedBreaths,
    pub dragon_fires: Vec<crate::dragon_fires::SavedDragonFire>,
    /// The Dark Elf's souls on their way and fading (#3809).
    pub souls: Vec<crate::souls::SavedSoul>,
    pub fading_souls: Vec<crate::souls::SavedFading>,
}

impl MapEntities for Things {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.arrows.map_entities(mapper);
        self.stuck.map_entities(mapper);
        self.bolts.map_entities(mapper);
        self.blasts.map_entities(mapper);
        self.shells.map_entities(mapper);
        self.hurled.map_entities(mapper);
        self.spits.map_entities(mapper);
        self.daggers.map_entities(mapper);
        self.casts.map_entities(mapper);
        self.summonings.map_entities(mapper);
        self.skulls.map_entities(mapper);
        self.spells.map_entities(mapper);
        self.breaths.map_entities(mapper);
        self.dragon_fires.map_entities(mapper);
        self.souls.map_entities(mapper);
        self.fading_souls.map_entities(mapper);
    }
}

/// The things in flight of `world`, their references kept by their names in `refs`.
fn things_of(world: &mut bevy::prelude::World, refs: &mut Refs) -> Things {
    let (arrows, stuck) = crate::arrows::saved(world);
    let bolts = crate::bolts::saved(world);
    let blasts = crate::blast::saved(world);
    let (souls, fading_souls) = crate::souls::saved(world);
    crate::names::mapped(
        Things {
            arrows,
            stuck,
            bolts,
            blasts,
            shells: crate::guns::saved(world),
            hurled: crate::hurled::saved(world),
            spits: crate::spits::saved(world),
            daggers: crate::creature_arts::saved_daggers(world),
            casts: crate::creature_casts::saved_objects(world),
            summonings: crate::followers::saved_summonings(world),
            skulls: crate::spells::saved_skulls(world),
            spells: crate::spells::saved_objects(world),
            breaths: crate::breath::saved(world),
            dragon_fires: crate::dragon_fires::saved(world),
            souls,
            fading_souls,
        },
        &mut Saving { world, refs },
    )
}

/// The saves' directory: `saves/` beside the settings' file, else in the configuration
/// directory.
pub(crate) fn directory(settings: Option<&SettingsFile>) -> Option<PathBuf> {
    settings
        .and_then(SettingsFile::path)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .or_else(crate::settings::config_dir)
        .map(|dir| dir.join(DIRECTORY))
}

/// The file of the save `name`: a name of letters, digits, blanks, `-` and `_`.
fn file(dir: &Path, name: &str) -> Result<PathBuf> {
    let name = name.trim();
    ensure!(
        !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_')),
        "{name:?}: a save's name is letters, digits, blanks, - and _"
    );
    Ok(dir.join(format!("{name}.{EXTENSION}")))
}

/// Writes a save of `header`, its `picture`, `hero`, `scripts`, `world` and `live` at `path`, the
/// picture after the header, which the saves' list reads with it.
pub fn write(
    path: &Path,
    (header, picture): (&Header, Option<&Picture>),
    (hero, scripts, world, live): (&Hero, &Scripts, &World, &Live),
) -> Result<()> {
    let mut bytes = postcard::to_stdvec(header).context("writing the header")?;
    bytes.extend(postcard::to_stdvec(&picture).context("writing the picture")?);
    bytes.extend(postcard::to_stdvec(hero).context("writing the hero")?);
    bytes.extend(postcard::to_stdvec(scripts).context("writing the scripts")?);
    bytes.extend(postcard::to_stdvec(world).context("writing the world")?);
    bytes.extend(postcard::to_stdvec(live).context("writing what lives")?);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
    }
    fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))
}

/// A save's header, its picture and the bytes after them, refused where its version is not
/// [`VERSION`].
fn header(bytes: &[u8]) -> Result<(Header, Option<Picture>, &[u8])> {
    let (header, rest) =
        postcard::take_from_bytes::<Header>(bytes).context("reading the header")?;
    let (picture, rest) = match header.version {
        VERSION => postcard::take_from_bytes(rest).context("reading the picture")?,
        version => bail!("a save of the version {version}, where the game reads {VERSION}"),
    };
    Ok((header, picture, rest))
}

/// Reads the save at `path`.
pub fn read(path: &Path) -> Result<(Header, Hero, Scripts, World, Live)> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let (header, _, rest) = header(&bytes).with_context(|| path.display().to_string())?;
    let (hero, rest) = postcard::take_from_bytes(rest)
        .with_context(|| format!("{}: reading the hero", path.display()))?;
    let (scripts, rest) = postcard::take_from_bytes(rest)
        .with_context(|| format!("{}: reading the scripts", path.display()))?;
    let (world, rest) = postcard::take_from_bytes(rest)
        .with_context(|| format!("{}: reading the world", path.display()))?;
    let live = postcard::from_bytes(rest)
        .with_context(|| format!("{}: reading what lives", path.display()))?;
    Ok((header, hero, scripts, world, live))
}

/// The saves in `dir`, each its file's name and its header, the newest first; a file that does
/// not read is left out.
pub fn list(dir: &Path) -> Vec<(String, Header)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut saves: Vec<(String, Header)> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == EXTENSION))
        .filter_map(|path| {
            let bytes = fs::read(&path).ok()?;
            let (header, _, _) = header(&bytes).ok()?;
            let stem = path.file_stem()?.to_string_lossy().into_owned();
            Some((stem, header))
        })
        .collect();
    saves.sort_by(|a, b| b.1.saved.cmp(&a.1.saved).then_with(|| a.0.cmp(&b.0)));
    saves
}

/// A slot's save as the saves' window lists it (#1990): its file's name, its header and its
/// picture, and whether it is of another version of the saves, which the window lists and does
/// not load (#3171).
#[derive(Clone, Debug)]
pub struct Slot {
    pub stem: String,
    pub header: Header,
    pub picture: Option<Picture>,
    pub stale: bool,
}

/// The header of a save of another version than [`VERSION`] (#3171): its own where it reads as
/// this version's layout, else one of the version it begins with, named by its file's `stem` and
/// dated by the file's time `saved`; none for a file of this version or of no version at all.
fn stale_header(bytes: &[u8], stem: &str, saved: u64) -> Option<Header> {
    if let Ok((header, _)) = postcard::take_from_bytes::<Header>(bytes) {
        return (header.version != VERSION).then_some(header);
    }
    let (version, _) = postcard::take_from_bytes::<u32>(bytes).ok()?;
    (version != VERSION).then(|| Header {
        version,
        name: stem.to_owned(),
        class: Class::default(),
        hero: String::new(),
        difficulty: Difficulty::default(),
        level: 0,
        clock: 0.0,
        saved,
        tree: None,
    })
}

/// Whether `stem` names a slot's file: `GAME` and digits as its fifth and sixth characters
/// (`0x71EAA0`), the slot 0 the quick save's.
fn slot_of(stem: &str) -> Option<u32> {
    let rest = stem.strip_prefix("GAME")?;
    let digits = rest.get(..2)?;
    digits
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| rest.parse().ok())
        .flatten()
}

/// The slots' saves in `dir` (`0x71EAA0`): the quick save of the slot 0 apart, the others by
/// their files' names as text, as the original sorts them; a save of another version among them
/// ([`stale_header`], #3171), a file that does not read at all left out.
pub fn slots(dir: &Path) -> (Option<Slot>, Vec<Slot>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return (None, Vec::new());
    };
    let mut slots: Vec<(u32, Slot)> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == EXTENSION))
        .filter_map(|path| {
            let stem = path.file_stem()?.to_string_lossy().into_owned();
            let slot = slot_of(&stem)?;
            let bytes = fs::read(&path).ok()?;
            let slot_of = |header, picture, stale| Slot {
                stem: stem.clone(),
                header,
                picture,
                stale,
            };
            if let Ok((header, picture, _)) = header(&bytes) {
                return Some((slot, slot_of(header, picture, false)));
            }
            let saved = fs::metadata(&path)
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |since| since.as_secs());
            let stale = stale_header(&bytes, &stem, saved)?;
            Some((slot, slot_of(stale, None, true)))
        })
        .collect();
    slots.sort_by(|a, b| a.1.stem.cmp(&b.1.stem));
    let quick = slots
        .iter()
        .position(|(slot, _)| *slot == 0)
        .map(|at| slots.remove(at).1);
    (quick, slots.into_iter().map(|(_, slot)| slot).collect())
}

/// The saves' directory of this run.
pub fn saves_directory(world: &bevy::prelude::World) -> Option<PathBuf> {
    directory(world.get_resource::<SettingsFile>())
}

/// Erases the save `stem` of `dir` (the saves' window's Delete, `0x71CC90`'s action 8).
pub fn erase(dir: &Path, stem: &str) -> Result<()> {
    let path = file(dir, stem)?;
    fs::remove_file(&path).with_context(|| format!("erasing {}", path.display()))?;
    info!("saves: {} erased", path.display());
    Ok(())
}

/// The hero and the game as they stand, to save as `name`.
pub(crate) fn gather(
    world: &mut bevy::prelude::World,
    name: &str,
    refs: &mut Refs,
) -> Result<(Header, Hero)> {
    let request = world
        .get_resource::<crate::game::Request>()
        .map(|request| request.0.clone())
        .unwrap_or_default();
    let mut heroes = world.query_filtered::<(
        &Transform,
        &Body,
        &crate::fight::Base,
        &Health,
        (
            Option<&Effects>,
            Option<&crate::npcs::ScriptQueue>,
            Option<&crate::attack::ShotChain>,
            Option<&crate::fight::KnockMarks>,
        ),
    ), With<Body>>();
    let (transform, body, base, health, (effects, queue, chain, marks)) =
        heroes.iter(world).next().context("no hero to save")?;
    let chain = chain.map_or(1.0, |chain| chain.0);
    let marks = marks.map_or(0, |marks| marks.0);
    // A dead hero, which rises at its home, is not saved: its save would load it dead.
    ensure!(health.current > 0, "the hero is dead");
    let floor = world
        .get_resource::<Heights>()
        .and_then(|heights| heights.floor(body.level()))
        .map_or(0, |(_, floor)| floor);
    let now = world
        .get_resource::<Time<Real>>()
        .map_or(0.0, Time::<Real>::elapsed_secs_f64);
    let heading = crate::npcs::heading_of(transform.rotation);
    // Its walk under way, but one its scripts' queue gives it, which the queue gives again.
    let walk = body
        .target()
        .filter(|_| queue.is_none_or(crate::npcs::ScriptQueue::is_empty))
        .map(|target| Walk::of(target, body.onto(), world.get_resource::<Heights>()));
    let (at, base, health, effects, queue) = (
        (transform.translation.x, transform.translation.z),
        base.0,
        health.current,
        crate::names::mapped(
            effects.cloned().unwrap_or_default(),
            &mut Saving { world, refs },
        ),
        queue.map(|queue| queue.saved(now)).unwrap_or_default(),
    );
    let resource = |what: &str| format!("no {what} to save");
    let experience = *world
        .get_resource::<crate::fight::Experience>()
        .with_context(|| resource("experience"))?;
    let difficulty = world
        .get_resource::<crate::game::Difficulty>()
        .map(|d| d.0)
        .unwrap_or_default();
    let clock = world
        .get_resource::<Clock>()
        .cloned()
        .with_context(|| resource("clock"))?;
    let zoom = world
        .query_filtered::<&Projection, crate::iso::Eye>()
        .iter(world)
        .find_map(|projection| match projection {
            Projection::Orthographic(ortho) => Some(ortho.scale),
            _ => None,
        })
        .unwrap_or(1.0);
    let hero = Hero {
        at,
        floor,
        heading,
        experience: experience.0,
        base,
        health,
        effects,
        arts: world
            .get_resource::<arts::HeroArts>()
            .map(|arts| arts.arts.clone())
            .with_context(|| resource("arts"))?,
        slots: world
            .get_resource::<ArtSlots>()
            .cloned()
            .unwrap_or_default(),
        selected: world.get_resource::<making::Selected>().and_then(|s| s.0),
        combos: world
            .get_resource::<HeroCombos>()
            .cloned()
            .unwrap_or_default(),
        pack: world
            .get_resource::<Pack>()
            .map(Pack::kept)
            .with_context(|| resource("pack"))?,
        sets: world
            .get_resource::<WeaponSets>()
            .cloned()
            .unwrap_or_default(),
        home: world.get_resource::<HeroHome>().and_then(|home| home.0),
        tally: world.get_resource::<Tally>().copied().unwrap_or_default(),
        kills: world.get_resource::<Kills>().cloned().unwrap_or_default(),
        history: world.get_resource::<History>().cloned().unwrap_or_default(),
        wins: world.get_resource::<Wins>().copied().unwrap_or_default(),
        runes_given: world
            .get_resource::<crate::rewards::RunesGiven>()
            .map_or(0, |given| given.0),
        quests: world
            .get_resource::<crate::rewards::QuestCounts>()
            .map(|counts| counts.0)
            .unwrap_or_default(),
        survival: world
            .get_resource::<drops::Survival>()
            .map_or(0.0, drops::Survival::seconds),
        clock: clock.clone(),
        queue,
        chain,
        marks,
        zoom,
        walk,
        windows: Windows {
            book: world
                .get_resource::<crate::quest_book::Book>()
                .is_some_and(|book| book.shown),
            inventory: world
                .get_resource::<crate::inventory::Shown>()
                .is_some_and(|shown| shown.0),
        },
    };
    let header = Header {
        version: VERSION,
        name: name.trim().to_owned(),
        class: request.class.unwrap_or_default(),
        hero: request
            .name
            .clone()
            .unwrap_or_else(|| request.class.unwrap_or_default().name().to_owned()),
        difficulty,
        level: experience.0.level,
        clock: clock.minutes() * 60.0,
        saved: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs()),
        tree: request.tree.clone(),
    };
    Ok((header, hero))
}

/// The scripts as they stand, to save.
fn scripts(world: &bevy::prelude::World) -> Result<Scripts> {
    let seed = world
        .get_resource::<crate::seed::Game>()
        .context("no game's seed to save")?;
    let live = world
        .get_non_send::<crate::scripts::Live>()
        .context("the scripts do not run")?;
    let npcs = world
        .get_resource::<crate::npcs::Npcs>()
        .context("no NPCs to save")?;
    Ok(Scripts {
        seed: seed.0,
        scripts: live.saved(),
        npcs: npcs.saved(),
    })
}

/// The world as it stands, to save.
fn world_of(world: &mut bevy::prelude::World, refs: &mut Refs) -> Result<World> {
    let creatures = crate::npcs::creatures(world).context("no NPCs to save")?;
    // The followers' and the horse's effects' sources by their names (#3755).
    let followers = crate::npcs::following(world);
    let horse = crate::npcs::mount_of(world);
    let (followers, horse) = {
        let mut saving = Saving { world, refs };
        (
            crate::names::mapped(followers, &mut saving),
            horse.map(|horse| crate::names::mapped(horse, &mut saving)),
        )
    };
    let bridle = crate::riding::bridle_of(world);
    let ridden = horse.is_some() && crate::riding::rides_own(world);
    let drops = drops::laid(world);
    // The weather's timers from the save's time of real time, which a load's run starts anew.
    let now = world
        .get_resource::<Time<Real>>()
        .map_or(0.0, |time| time.elapsed_secs_f64() * 1_000.0);
    let weather = world
        .get_resource::<crate::weather::Weather>()
        .map(|weather| {
            let mut state = weather.state.clone();
            state.rebase(-now);
            state
        });
    Ok(World {
        creatures,
        followers,
        horse,
        bridle,
        ridden,
        drops,
        stash: world.get_resource::<Chest>().cloned().unwrap_or_default(),
        wares: world
            .get_resource::<Wares>()
            .map(|wares| wares.0.clone())
            .unwrap_or_default(),
        land: world
            .get_resource::<Land>()
            .map(|land| land.0.clone())
            .unwrap_or_default(),
        sectors: world
            .get_resource::<MapSectors>()
            .map(|sectors| sectors.0.clone())
            .unwrap_or_default(),
        pin: world.get_resource::<Map>().and_then(Map::pinned),
        weather,
        stalls: crate::stables::saved_stalls(world),
        buildings: world
            .get_resource::<crate::buildings::Buildings>()
            .map(crate::buildings::Buildings::saved)
            .unwrap_or_default(),
        fields: crate::pill_fields::saved(world),
        fences: crate::laser_fences::saved(world),
        arrows: crate::beacons::saved(world),
    })
}

/// Saves the game as `name`: where its file went.
pub fn save(world: &mut bevy::prelude::World, name: &str) -> Result<PathBuf> {
    save_in(world, name, name)
}

/// Saves the game as `name` in the file `stem` of the saves' directory, as the original's save
/// keeps its name inside its slot's file (`GAME%.2d.PAK`, docs/re/saves.md).
pub(crate) fn save_in(world: &mut bevy::prelude::World, stem: &str, name: &str) -> Result<PathBuf> {
    let dir = directory(world.get_resource::<SettingsFile>()).context("no directory for saves")?;
    let path = file(&dir, stem)?;
    let mut refs = Refs::default();
    let (header, hero) = gather(world, name, &mut refs)?;
    let scripts = scripts(world)?;
    let kept = world_of(world, &mut refs)?;
    let doings = crate::doings::gather(world, &mut refs);
    let things = things_of(world, &mut refs);
    let bodies = crate::npcs::bodies(world);
    let animals = crate::npcs::animals(world);
    let specials = Specials {
        slaves: crate::shaddar_slaves::saved(world),
        illusions: crate::demon_illusions::saved(world),
        zhadums: crate::zhadums::saved(world),
    };
    let specials = crate::names::mapped(
        specials,
        &mut Saving {
            world,
            refs: &mut refs,
        },
    );
    let hurled = world
        .get_resource::<crate::arsenal::Arsenal>()
        .is_some_and(crate::arsenal::Arsenal::thrown);
    let looks = Looks {
        puffs: crate::puffs::saved(world),
        impacts: crate::grenade_impacts::saved(world),
        rays: crate::hard_rays::saved(world),
        chain_bolts: crate::chain_bolts::saved(world),
        trails: crate::bolt_trails::saved(world).1,
        flashes: crate::gun_fires::saved(world),
        beams: crate::beams::saved(world),
        rings: crate::arrow_sights::saved(world),
        streaks: crate::weapon_streaks::saved(world),
        blasts: crate::explosions::saved(world),
        meteors: crate::meteors::saved(world),
        light_strikes: crate::light_strikes::saved(world),
        sera_sights: crate::sera_sights::saved(world),
        gusts: crate::twisters::saved(world),
        splinters: crate::ice_splinters::saved(world).1,
        floops: crate::floops::saved(world),
        elf_sights: crate::elf_sights::saved(world),
        cages: crate::plant_cages::saved(world),
        healings: crate::plant_healings::saved(world),
        wards: crate::dark_wards::saved(world),
        helices: crate::dark_helices::saved(world),
        armours: crate::dwarf_armours::saved(world),
        berserks: crate::dwarf_berserks::saved(world),
        burrows: crate::dwarf_burrows::saved(world),
        greeds: crate::dwarf_greeds::saved(world),
        cry_signs: crate::war_cry_signs::saved(world),
        gun_sights: crate::dwarf_fires::saved(world),
        forces: crate::hell_forces::saved(world),
        deaths: crate::demon_deaths::saved(world),
        forms: crate::demon_forms::saved(world),
        glad_sight: crate::glad_sights::saved(world),
        cries: crate::war_cries::saved(world),
        knockouts: crate::knockouts::saved(world),
        doom_signs: crate::doom_signs::saved(world),
        teleports: crate::teleports::saved(world),
        stones: crate::stones::saved(world),
        reanimations: crate::reanimations::saved(world),
        burning_bones: crate::burning_bones::saved(world),
        goblin_impacts: crate::goblin_impacts::saved(world),
        runes: crate::banishing_runes::saved(world),
        battle_fogs: crate::battle_fogs::saved(world),
        healing_sights: crate::healings::saved(world),
        speed_sights: crate::speed_sights::saved(world),
        holding_sights: crate::holding_sights::saved(world),
        trap_balls: crate::trap_balls::saved(world),
        trap_landings: crate::trap_landings::saved(world),
        homing_fires: crate::homing_fires::saved(world),
        flying_skulls: crate::flying_skulls::saved(world),
        shields: crate::shields::saved(world),
        cast_sights: crate::cast_sights::saved(world),
        bubbles: crate::bubble_traps::saved(world).0,
        sonars: crate::bubble_traps::saved(world).1,
        bubble_prisons: crate::bubble_prisons::saved(world),
        prisons: crate::prisons::saved(world),
        stargates: crate::stargates::saved(world),
        vamp_sights: crate::vamp_sights::saved(world),
        bite_sights: crate::bite_sights::saved(world),
        vamp_casts: crate::vamp_casts::saved(world),
        soul_sparks: crate::soul_sparks::saved(world),
        soulcatchers: crate::soulcatchers::saved(world),
        marks: crate::marks::saved(world),
        fountains: crate::blood_fountains::saved(world),
        body_parts: crate::body_parts::saved(world),
        blows: crate::blow_sights::saved(world),
        summon_looks: crate::summon_looks::saved(world),
        summonings: crate::summonings::saved(world),
        lightnings: crate::lightning_strikes::saved(world),
        poison_rings: crate::poison_rings::saved(world),
        spark_lamps: crate::hit_sparks::saved(world),
    };
    let looks = crate::names::mapped(
        looks,
        &mut Saving {
            world,
            refs: &mut refs,
        },
    );
    let live = Live {
        refs,
        doings,
        things,
        bodies,
        animals,
        specials,
        hurled,
        looks,
    };
    // The picture last taken (`save_picture`), as the original's save keeps its last capture.
    let picture = world
        .get_resource::<crate::save_picture::Taken>()
        .and_then(|taken| taken.picture.clone());
    write(
        &path,
        (&header, picture.as_ref()),
        (&hero, &scripts, &kept, &live),
    )?;
    info!("saves: {} saved in {}", header.name, path.display());
    Ok(path)
}

/// The saves of this run's directory.
pub fn saves(world: &bevy::prelude::World) -> Vec<(String, Header)> {
    directory(world.get_resource::<SettingsFile>())
        .map(|dir| list(&dir))
        .unwrap_or_default()
}

/// A saved hero and world to put back once the loaded game's hero stands.
#[derive(Resource)]
pub struct Restore(pub Box<Hero>, pub Box<World>, pub Box<Live>);

/// A loaded save's scripts and what its sectors kept of their creatures, which its game's load
/// takes ([`crate::game`]'s `begin`).
#[derive(Resource)]
pub struct Resume(pub Scripts, pub crate::npcs::Creatures);

/// Loads the save `name`: a game of its hero starts of its scripts and its sectors' creatures,
/// whose hero and world [`restore`] puts back.
pub fn load(world: &mut bevy::prelude::World, name: &str) -> Result<Header> {
    let dir = directory(world.get_resource::<SettingsFile>()).context("no directory for saves")?;
    let (header, hero, scripts, mut kept, live) = read(&file(&dir, name)?)?;
    let shows = world
        .get_resource::<crate::game::Shows>()
        .copied()
        .unwrap_or_default();
    let game = game_of(&header, &hero, shows);
    // Its start shows no window, and the saved game's are left to the player.
    world.remove_resource::<crate::talk::CloseFirst>();
    world.insert_resource(crate::game::Request(game));
    let creatures = std::mem::take(&mut kept.creatures);
    // The hero of a game that the load leaves is not the save's: [`restore`] waits for the load's
    // own, which `hero::leave` would mark only a frame later, as the state changes.
    if let Some(mut ready) = world.get_resource_mut::<crate::hero::Ready>() {
        ready.0 = false;
    }
    world.insert_resource(Restore(Box::new(hero), Box::new(kept), Box::new(live)));
    world.insert_resource(Resume(scripts, creatures));
    world
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Loading);
    info!("saves: loading {}", header.name);
    Ok(header)
}

/// The game that a load of the save of `header` and `hero` starts: its hero's in the hero's
/// sector, which [`restore`] puts it back in, at the save's zoom where the run's `--zoom`
/// (`shows`) names none (#3740).
fn game_of(header: &Header, hero: &Hero, shows: crate::game::Shows) -> NewGame {
    let cell = crate::paths::cell_of(hero.at.0, hero.at.1);
    let side = i32::try_from(opensacred_data::world::SIDE).unwrap_or(64);
    let sector = |c: i32| u32::try_from(c.div_euclid(side)).unwrap_or(0);
    let mut game = shows.fill(NewGame {
        class: Some(header.class),
        name: Some(header.hero.clone()),
        difficulty: Some(header.difficulty),
        sector: Some((sector(cell.0), sector(cell.1))),
        tree: header.tree.clone(),
        ..NewGame::default()
    });
    game.zoom = game.zoom.or(Some(hero.zoom));
    game
}

/// Puts a loaded save's hero back, once its hero stands in place and animates (`hero::Ready`),
/// its values made at its spawn: its values, items, arts, combos, statistics and the calendar,
/// after the resets of a game's start, and the hero where it stood; then its world
/// ([`restore_world`]).
fn restore(world: &mut bevy::prelude::World) {
    let mut heroes = world.query_filtered::<Entity, With<Body>>();
    let Some(entity) = heroes.iter(world).next() else {
        return;
    };
    let Some(Restore(hero, kept, live)) = world.remove_resource::<Restore>() else {
        return;
    };
    let hero = *hero;
    // Its effects' sources, the creatures that bear their names now (#3755).
    let (mut effects, lost) = loaded(world, &live.refs, hero.effects.clone());
    effects.forget_lost();
    if lost > 0 {
        info!("saves: {lost} of the hero's effects' sources are not in the world");
    }
    bevy::log::debug!(
        "saves: the hero's effects' sources {:?}",
        effects
            .timed()
            .iter()
            .filter_map(|timed| timed.source)
            .collect::<Vec<_>>()
    );
    if let Ok(mut entity) = world.get_entity_mut(entity) {
        entity.insert((effects, crate::attack::ShotChain(hero.chain)));
        if hero.marks == 0 {
            entity.remove::<crate::fight::KnockMarks>();
        } else {
            entity.insert(crate::fight::KnockMarks(hero.marks));
        }
    }
    put_character(world, entity, &hero);
    world.insert_resource(HeroHome(hero.home));
    world.insert_resource(hero.clock);
    // The scripts' timers count on from the clock put back, not the minutes across its change.
    if let Some(mut live) = world.get_non_send_mut::<crate::scripts::Live>() {
        live.clock_set();
    }
    let position = Vec3::new(hero.at.0, 0.0, hero.at.1);
    if let Err(e) = crate::command::teleport(world, position, Arrival::On(Some(hero.floor))) {
        warn!("saves: putting the hero back: {e}");
    }
    // Facing as it faced (#3744).
    if let Some(mut transform) = world.get_mut::<Transform>(entity) {
        transform.rotation = crate::hero::facing(crate::npcs::heading(hero.heading));
    }
    // The steps of its queue that the save kept (#2890).
    hero.queue.clone().give_now(world, entity);
    // The walk under way, once it has landed (#3742).
    if let Some(walk) = hero.walk {
        world.insert_resource(WalkOn(walk));
    }
    // The windows that showed (#3741).
    if hero.windows.book
        && let Some(mut book) = world.get_resource_mut::<crate::quest_book::Book>()
    {
        book.show_again();
    }
    if hero.windows.inventory {
        world.insert_resource(crate::inventory::Shown(true));
    }
    info!("saves: the hero is back");
    restore_world(world, *kept, (hero.health, &live.refs));
    // What lives, once the loaded scene is whole ([`resume`]).
    world.insert_resource(Resuming {
        live: *live,
        frames: 0,
    });
}

/// Sends a loaded hero on to where it was going ([`Walk`], #3742), once it has landed where the
/// load put it back.
fn walk_on(
    mut commands: Commands,
    walk: Res<WalkOn>,
    heights: Option<Res<Heights>>,
    mut bodies: Query<&mut Body>,
) {
    let (Some(heights), Ok(mut body)) = (heights, bodies.single_mut()) else {
        return;
    };
    if body.landing() {
        return;
    }
    let (to, level) = walk.0.order(&heights);
    body.send_to(to, level);
    commands.remove_resource::<WalkOn>();
    info!("saves: the hero walks on to {to}");
}

/// Mounts a loaded hero on the own horse it rode at the save ([`RideOn`], #3743), once it has
/// landed where the load put it back, and once it rides gives it back its health as saved, which
/// the horse under it raises the most of.
fn ride_on(world: &mut bevy::prelude::World) {
    let Some(&RideOn { horse, health }) = world.get_resource::<RideOn>() else {
        return;
    };
    let mut heroes = world.query_filtered::<(
        Entity,
        &Body,
        Option<&crate::riding::Seat>,
        bevy::prelude::Has<crate::riding::Saddle>,
    ), With<Body>>();
    let Some((me, body, seat, saddled)) = heroes.iter(world).next() else {
        return;
    };
    if body.landing() {
        return;
    }
    match seat.copied() {
        None if world.get_entity(horse).is_ok() => crate::riding::seat_again(world, me, horse),
        Some(seat) if seat.ridden() == Some(horse) && saddled => {
            if let Some(mut now) = world.get_mut::<Health>(me) {
                now.current = health.min(now.most);
            }
            world.remove_resource::<RideOn>();
        }
        Some(crate::riding::Seat::Mounting { .. }) => {}
        _ => {
            world.remove_resource::<RideOn>();
        }
    }
}

/// A loaded save's living part to give back once the loaded scene is whole ([`resume`]), and the
/// frames waited for it.
#[derive(Resource)]
pub struct Resuming {
    live: Live,
    frames: u32,
}

/// Whether the game's time stands for a load (#3756), which [`resume`] lets run again.
#[derive(Resource, Default)]
struct Held(bool);

/// The frames that [`resume`] waits at most for the creatures of the saved doings to come in.
const RESUME_WAIT: u32 = 300;

/// Stands the game's time from a load's first frame in the game until [`resume`], without the
/// pause's menu (`pause::Still`), so that nothing thinks or moves before what lives is back: the
/// creatures of the hero's sectors would otherwise act from where the start made them.
fn hold(
    mut held: ResMut<Held>,
    play: Option<Res<bevy::prelude::State<crate::pause::Play>>>,
    mut next: ResMut<NextState<crate::pause::Play>>,
    mut still: ResMut<crate::pause::Still>,
) {
    let running = play.is_some_and(|play| *play.get() == crate::pause::Play::Running);
    if held.0 || !running {
        return;
    }
    held.0 = true;
    still.0 = true;
    next.set(crate::pause::Play::Paused);
}

/// Gives a loaded save's creatures their doings back once the hero is back and the sectors about
/// it are in, and every creature of a doing has come in or [`RESUME_WAIT`] frames have passed
/// (#3756), then lets the game's time run that [`hold`] stood.
fn resume(world: &mut bevy::prelude::World) {
    let Some(mut resuming) = world.get_resource_mut::<Resuming>() else {
        return;
    };
    resuming.frames += 1;
    let frames = resuming.frames;
    let mut heroes = world.query_filtered::<&Transform, With<Body>>();
    let Some(hero) = heroes.iter(world).next().map(|at| at.translation) else {
        return;
    };
    let settled = world
        .get_resource::<crate::sectors::Streamer>()
        .is_none_or(|streamer| streamer.settled(hero));
    // The changelings' animals, once the hero's sectors are in, under the names that the doings
    // wait for (#3767).
    if settled || frames >= RESUME_WAIT {
        let animals = world
            .get_resource_mut::<Resuming>()
            .map(|mut resuming| std::mem::take(&mut resuming.live.animals))
            .unwrap_or_default();
        if !animals.is_empty() {
            crate::npcs::animals_again(world, animals);
        }
    }
    let empty = Names::default();
    let names = world.get_resource::<Names>().unwrap_or(&empty);
    let Some(resuming) = world.get_resource::<Resuming>() else {
        return;
    };
    let come = resuming
        .live
        .doings
        .owners()
        .all(|id| names.get(id).is_some());
    if !(settled && come) && frames < RESUME_WAIT {
        return;
    }
    let Some(Resuming { live, .. }) = world.remove_resource::<Resuming>() else {
        return;
    };
    // The bodies first, which the things' arrows may stick in.
    crate::npcs::lay_again(world, live.bodies);
    let (doings, lost) = loaded(world, &live.refs, live.doings);
    let (given, missing) = crate::doings::give(world, doings);
    // The Vampiress's night's form, which her effects kept (#3768).
    crate::forms::wear_again(world);
    // The things in flight, once the creatures they refer to are back.
    let (things, lost_things) = loaded(world, &live.refs, live.things);
    let lost = lost + lost_things;
    crate::arrows::again(world, (things.arrows, things.stuck));
    crate::bolts::again(world, things.bolts);
    crate::blast::again(world, things.blasts);
    crate::guns::again(world, things.shells);
    crate::hurled::again(world, things.hurled);
    crate::spits::again(world, things.spits);
    crate::creature_arts::daggers_again(world, things.daggers);
    crate::creature_casts::objects_again(world, things.casts);
    crate::followers::summonings_again(world, things.summonings);
    crate::spells::skulls_again(world, things.skulls);
    crate::spells::objects_again(world, things.spells);
    crate::breath::again(world, things.breaths);
    crate::dragon_fires::again(world, things.dragon_fires);
    crate::souls::again(world, (things.souls, things.fading_souls));
    // The looks, last, once what they are drawn at is back (#3773).
    let (looks, lost_looks) = loaded(world, &live.refs, live.looks);
    let lost = lost + lost_looks;
    crate::puffs::again(world, looks.puffs);
    crate::grenade_impacts::again(world, looks.impacts);
    crate::hard_rays::again(world, looks.rays);
    crate::chain_bolts::again(world, looks.chain_bolts);
    crate::bolt_trails::again(world, looks.trails);
    crate::gun_fires::again(world, looks.flashes);
    crate::beams::again(world, looks.beams);
    crate::arrow_sights::again(world, looks.rings);
    crate::weapon_streaks::again(world, looks.streaks);
    crate::explosions::again(world, looks.blasts);
    crate::meteors::again(world, looks.meteors);
    crate::light_strikes::again(world, looks.light_strikes);
    crate::sera_sights::again(world, looks.sera_sights);
    crate::twisters::again(world, looks.gusts);
    crate::ice_splinters::again(world, looks.splinters);
    crate::floops::again(world, looks.floops);
    crate::elf_sights::again(world, looks.elf_sights);
    crate::plant_cages::again(world, looks.cages);
    crate::plant_healings::again(world, looks.healings);
    crate::dark_wards::again(world, looks.wards);
    crate::dark_helices::again(world, looks.helices);
    crate::dwarf_armours::again(world, looks.armours);
    crate::dwarf_berserks::again(world, looks.berserks);
    crate::dwarf_burrows::again(world, looks.burrows);
    crate::dwarf_greeds::again(world, looks.greeds);
    crate::war_cry_signs::again(world, looks.cry_signs);
    crate::dwarf_fires::again(world, looks.gun_sights);
    crate::hell_forces::again(world, looks.forces);
    crate::demon_deaths::again(world, looks.deaths);
    crate::demon_forms::again(world, looks.forms);
    crate::glad_sights::again(world, looks.glad_sight);
    crate::war_cries::again(world, looks.cries);
    crate::knockouts::again(world, looks.knockouts);
    crate::doom_signs::again(world, looks.doom_signs);
    crate::teleports::again(world, looks.teleports);
    crate::stones::again(world, looks.stones);
    crate::reanimations::again(world, looks.reanimations);
    crate::burning_bones::again(world, looks.burning_bones);
    crate::goblin_impacts::again(world, looks.goblin_impacts);
    crate::banishing_runes::again(world, looks.runes);
    crate::battle_fogs::again(world, looks.battle_fogs);
    crate::healings::again(world, looks.healing_sights);
    crate::speed_sights::again(world, looks.speed_sights);
    crate::holding_sights::again(world, looks.holding_sights);
    crate::trap_balls::again(world, looks.trap_balls);
    crate::trap_landings::again(world, looks.trap_landings);
    crate::homing_fires::again(world, looks.homing_fires);
    crate::flying_skulls::again(world, looks.flying_skulls);
    crate::shields::again(world, looks.shields);
    crate::cast_sights::again(world, looks.cast_sights);
    crate::bubble_traps::again(world, looks.bubbles, looks.sonars);
    crate::bubble_prisons::again(world, looks.bubble_prisons);
    crate::prisons::again(world, looks.prisons);
    crate::stargates::again(world, looks.stargates);
    crate::vamp_sights::again(world, looks.vamp_sights);
    crate::bite_sights::again(world, looks.bite_sights);
    crate::vamp_casts::again(world, looks.vamp_casts);
    crate::soul_sparks::again(world, looks.soul_sparks);
    crate::soulcatchers::again(world, looks.soulcatchers);
    crate::marks::again(world, looks.marks);
    crate::blood_fountains::again(world, looks.fountains);
    crate::body_parts::again(world, looks.body_parts);
    crate::blow_sights::again(world, looks.blows);
    crate::summon_looks::again(world, looks.summon_looks);
    crate::summonings::again(world, looks.summonings);
    crate::lightning_strikes::again(world, looks.lightnings);
    crate::poison_rings::again(world, looks.poison_rings);
    crate::hit_sparks::again(world, looks.spark_lamps);
    // The hero's hurled weapon out of his hand, flying or lying (#3769).
    if live.hurled {
        world.write_message(crate::arsenal::Throws(true));
    }
    // The scripts' special objects, once the creatures they hunt are back (#3768).
    let (specials, lost_specials) = loaded(world, &live.refs, live.specials);
    let lost = lost + lost_specials;
    crate::shaddar_slaves::again(world, specials.slaves);
    crate::demon_illusions::again(world, specials.illusions);
    crate::zhadums::again(world, specials.zhadums);
    info!(
        "saves: what lives is back after {frames} frames: {given} doings, {missing} creatures \
         missing, {lost} references lost"
    );
    if world.get_resource::<Held>().is_some_and(|held| held.0) {
        world.insert_resource(Held(false));
        world
            .resource_mut::<NextState<crate::pause::Play>>()
            .set(crate::pause::Play::Running);
    }
    // The fixed systems that the sectors made anew run on at the first frame played (#3805).
    if let Some(mut preroll) = world.get_resource_mut::<crate::fx::Preroll>() {
        preroll.0 = true;
    }
}

/// Puts the character of `hero` on the hero `entity` (#1095, #2552): its values and health at its
/// level, its experience and points, combat arts and their slots, combos, pack and weapon sets, its
/// statistics and wins and the survival counter, its values made anew with them; not where it
/// stands, its home, its effects or the clock, which a save puts back and an import does not.
fn put_character(world: &mut bevy::prelude::World, entity: Entity, hero: &Hero) {
    if let Ok(mut entity) = world.get_entity_mut(entity) {
        if let Some(mut base) = entity.get_mut::<crate::fight::Base>() {
            base.0 = hero.base;
        }
        // Its health at its level, so that no level-up fills it (`fight::level_up`).
        if let Some(mut health) = entity.get_mut::<Health>() {
            health.current = hero.health;
            health.level = hero.experience.level;
        }
    }
    // The taskbar's "+" as the hero's points left it (`0x585260`, #1992).
    world.insert_resource(crate::points::Plus::loaded(&hero.experience));
    world.insert_resource(crate::fight::Experience(hero.experience));
    // Put back, not gained: no line of it over the hero.
    world.insert_resource(crate::rewards::Unearned(true));
    if let Some(mut arts) = world.get_resource_mut::<arts::HeroArts>() {
        arts.arts = hero.arts.clone();
    }
    // The day's bank of the art slots, as the hero's form loads (`0x6058A0` at `0x60618E`).
    let mut slots = hero.slots.clone();
    slots.turn(false);
    world.insert_resource(slots);
    world.insert_resource(making::Selected(hero.selected));
    world.insert_resource(hero.combos.clone());
    if let Some(mut pack) = world.get_resource_mut::<Pack>() {
        pack.restore(hero.pack.clone());
    }
    world.insert_resource(hero.sets.clone());
    world.insert_resource(hero.tally);
    world.insert_resource(hero.kills.clone());
    world.insert_resource(hero.history.clone());
    world.insert_resource(hero.wins);
    world.insert_resource(crate::rewards::RunesGiven(hero.runes_given));
    world.insert_resource(crate::rewards::QuestCounts(hero.quests));
    world.insert_resource(drops::Survival::of(hero.survival));
    if let Err(e) = world.run_system_cached(refit) {
        warn!("saves: making the hero's values anew: {e}");
    }
}

/// An exported hero to put into the game once its hero stands ([`import`], #2552).
#[derive(Resource)]
pub struct Import(pub Box<Hero>);

/// Puts an exported hero's character on the game's hero once it stands (`IMPORT_DO`'s hero's
/// file over the hero of its class, `0x70DC20`; docs/re/underworld.md), where the game's start
/// placed it, its clock the game's own.
fn import(world: &mut bevy::prelude::World) {
    let mut heroes = world.query_filtered::<Entity, With<Body>>();
    let Some(entity) = heroes.iter(world).next() else {
        return;
    };
    let Some(Import(hero)) = world.remove_resource::<Import>() else {
        return;
    };
    put_character(world, entity, &hero);
    info!(
        "saves: the exported hero stands, of level {}",
        hero.experience.level
    );
}

/// Puts a loaded save's world back after the resets of a game's start, its sectors' creatures put
/// back as they come in (`npcs::Npcs::remember`): the town stash, the merchants' stock, the land
/// explored, the map's pin and the weather, its timers on this run's real time; the drops, laid
/// once the tables are read (`drops::Again`); the hero's followers beside it; its own horse
/// where it stood, with its bridle (`npcs::mount_again`, `riding::bridle_again`); and the scripts'
/// traps, put back onto those of their points (`pill_fields::Again`, `laser_fences::Again`); and
/// their gold arrows, in place of those that the load's start made (`beacons::again`).
/// `value` with the references that a save keeps by their names in `refs` mapped to the entities
/// of `world` that bear those names, and the count of those that none bears (`names::Loading`).
fn loaded<T: MapEntities>(world: &bevy::prelude::World, refs: &Refs, value: T) -> (T, usize) {
    let empty = Names::default();
    let names = world.get_resource::<Names>().unwrap_or(&empty);
    let mut loading = Loading {
        refs,
        names,
        lost: 0,
    };
    let value = crate::names::mapped(value, &mut loading);
    (value, loading.lost)
}

fn restore_world(world: &mut bevy::prelude::World, kept: World, (health, refs): (u32, &Refs)) {
    let World {
        creatures: _,
        followers,
        horse,
        bridle,
        ridden,
        drops: laid,
        stash,
        wares,
        land,
        sectors,
        pin,
        weather,
        stalls,
        buildings,
        fields,
        fences,
        arrows,
    } = kept;
    // The followers' and the horse's effects' sources, the creatures that bear their names now.
    let (mut followers, lost) = loaded(world, refs, followers);
    let (mut horse, lost_too) = loaded(world, refs, horse);
    followers
        .iter_mut()
        .chain(horse.iter_mut())
        .for_each(crate::npcs::Following::forget_lost);
    if lost + lost_too > 0 {
        info!(
            "saves: {} of the followers' effects' sources are not in the world",
            lost + lost_too
        );
    }
    world.insert_resource(stash);
    world.insert_resource(Wares(wares));
    world.insert_resource(Land(land));
    world.insert_resource(MapSectors(sectors));
    if let Some(mut map) = world.get_resource_mut::<Map>() {
        map.pin_again(pin);
    }
    let now = world
        .get_resource::<Time<Real>>()
        .map_or(0.0, |time| time.elapsed_secs_f64() * 1_000.0);
    if let (Some(mut state), Some(mut weather)) =
        (weather, world.get_resource_mut::<crate::weather::Weather>())
    {
        state.rebase(now);
        weather.state = state;
    }
    world.insert_resource(drops::Again(laid));
    crate::npcs::follow_again(world, followers);
    if let Some(horse) = horse {
        let made = crate::npcs::mount_again(world, horse);
        crate::riding::bridle_again(world, bridle);
        // A hero that rode it mounts it again once it has landed, its health as saved.
        if let (Some(horse), true) = (made, ridden) {
            world.insert_resource(RideOn { horse, health });
        }
    }
    crate::stables::stalls_again(world, stalls);
    if let Some(mut states) = world.get_resource_mut::<crate::buildings::Buildings>() {
        states.again(buildings);
    }
    world.insert_resource(crate::pill_fields::Again(fields));
    world.insert_resource(crate::laser_fences::Again(fences));
    crate::beacons::again(world, arrows);
    info!("saves: the world is back");
}

/// The loaded hero's values made anew at its level with its effects, as a point spent makes them
/// (`statistics::refit`): its attack, defence, damage, resistances, regeneration, most health
/// and its arts' skills.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn refit(
    experience: Res<crate::fight::Experience>,
    rules: Res<crate::fight::Rules>,
    mut heroes: Query<
        (
            &crate::fight::Base,
            Option<&crate::fight::Holding>,
            Option<&Effects>,
            &mut crate::fight::Combat,
            &mut crate::fight::Regenerates,
            &mut Health,
            Option<&crate::riding::Saddle>,
        ),
        With<Body>,
    >,
    (mut arts, mut hero): (
        Option<ResMut<arts::HeroArts>>,
        Option<ResMut<crate::hero::Hero>>,
    ),
) {
    for (base, holding, effects, mut combat, mut regenerates, mut health, saddle) in &mut heroes {
        crate::statistics::refit(
            experience.level,
            &rules,
            (base, holding, effects, saddle),
            (&mut combat, &mut regenerates, &mut health),
            (arts.as_deref_mut(), hero.as_deref_mut()),
        );
    }
}

/// The quick save's name and its file, the slot 0 (`cEngine_receiveEvent`, `0x618E3A`), which F8
/// loads.
pub const QUICK_SAVE: &str = "QUICKSAVE";
const QUICK_FILE: &str = "GAME00";

/// The file of the first slot from 1 that no save holds (`0x619B80`: `GAME01` and on, the slot
/// 511 where every one to 509 is taken), which an autosave takes.
pub(crate) fn free_slot(dir: &Path) -> String {
    (1..0x1FE_u16)
        .map(|slot| format!("GAME{slot:02}"))
        .find(|stem| !dir.join(format!("{stem}.{EXTENSION}")).exists())
        .unwrap_or_else(|| "GAME511".to_owned())
}

/// The numbers of the scripts' `AutoSave`s run (`scripts`; 121, `0x4AC740`; #1989), which
/// [`autosave`] saves the game by.
#[derive(Resource, Default)]
pub struct AutoSaves(pub Vec<u32>);

/// Saves the game where the scripts ran an `AutoSave` and the autosave is on (`[gameplay]
/// autosave`, the original's `AUTOSAVE`, `0x81B6F0`): into the first free slot, named by the text
/// of its number (`cEngine::save`, `0x619BF0`), whatever windows show, as the record asks the
/// engine without the autosave's test of the interface (`0x618FA0`).
fn autosave(world: &mut bevy::prelude::World) {
    let numbers = std::mem::take(&mut world.resource_mut::<AutoSaves>().0);
    let Some(&number) = numbers.last() else {
        return;
    };
    let on = world
        .get_resource::<crate::settings::Settings>()
        .is_some_and(|settings| settings.gameplay.autosave);
    if !on {
        info!("saves: the scripts' autosave {number} left, the autosave being off");
        return;
    }
    // Its picture first, as the record's `(0xF, 3)` comes before its save (`0x4AC740`).
    picture_first(world, Waits::Auto(number));
}

/// The autosave of `number`, its picture taken.
fn autosave_now(world: &mut bevy::prelude::World, number: u32) {
    let name = world
        .get_resource::<crate::lines::Texts>()
        .and_then(|texts| texts.named(&number.to_string()))
        .map_or_else(|| number.to_string(), str::to_owned);
    let Some(dir) = directory(world.get_resource::<SettingsFile>()) else {
        warn!("saves: no directory for the autosave {number}");
        return;
    };
    if let Err(e) = save_in(world, &free_slot(&dir), &name) {
        warn!("saves: the autosave {number}: {e:#}");
    }
}

/// The windows whose showing refuses a save (`0x75ADB0`): the merchant's, the smith's, the combat
/// master's and the town stash's (the players' trade, its fifth, the game has not), and a
/// dialogue's, one of the tooltips' windows of the manager's list `+0x124`, any of which shown
/// refuses it too (#2878; seen: the quick save writes no file while a talk's window shows).
pub type Refusing<'w> = (
    Option<Res<'w, crate::merchant::Merchant>>,
    Option<Res<'w, crate::blacksmith::Blacksmith>>,
    Option<Res<'w, crate::master::Master>>,
    Option<Res<'w, crate::town_stash::Opened>>,
    Option<Res<'w, crate::talk::Shown>>,
);

/// Whether the interface allows a save: none of the `refusing` windows shows (`0x75ADB0`).
pub fn allowed((merchant, smith, master, stash, dialogue): &Refusing) -> bool {
    merchant.is_none()
        && smith.is_none()
        && master.is_none()
        && stash.as_ref().is_none_or(|stash| stash.0.is_none())
        && dialogue.is_none()
}

/// A quick save or its load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Quick {
    Save,
    Load,
}

/// The quick save or load that its key asked for this frame, which [`quick`] carries out, as the
/// original's key sets the engine's request that its loop carries out (`+0x54` bit `0x40`).
#[derive(Resource, Default)]
struct Asked(Option<Quick>);

/// F9 and Control+Z ask for the quick save where the interface allows one, F8 and Control+X for
/// its load (`cEngine_receiveEvent`, `0x618090`; docs/re/saves.md, "When it saves").
fn quick_keys(mut keys: crate::controls::KeysMut, refusing: Refusing, mut asked: ResMut<Asked>) {
    if keys.take(crate::controls::Action::QuickSave) {
        if allowed(&refusing) {
            asked.0 = Some(Quick::Save);
        } else {
            info!("saves: no quick save while a trader's, a stash's or a dialogue's window shows");
        }
    } else if keys.take(crate::controls::Action::QuickLoad) {
        asked.0 = Some(Quick::Load);
    }
}

/// Carries out the quick save or load asked for, as the console's `save` and `load` do, the save
/// once its picture is taken, as F9 takes it right before (`0x618E3A`).
fn quick(world: &mut bevy::prelude::World) {
    let Some(asked) = world.resource_mut::<Asked>().0.take() else {
        return;
    };
    match asked {
        Quick::Save => picture_first(world, Waits::Quick),
        Quick::Load => {
            if let Err(e) = load(world, QUICK_FILE) {
                warn!("saves: the quick load: {e:#}");
            }
        }
    }
}

/// A save that waits for its picture (#1991): the quick save or an autosave of its number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Waits {
    Quick,
    Auto(u32),
}

/// The save waiting for its picture, the pictures taken as it began to wait, and the frames it
/// has waited.
#[derive(Resource, Default)]
struct Waiting(Option<(Waits, u32, u32)>);

/// The frames a save waits for its picture at most, after which it saves with the last taken.
const PICTURE_WAIT: u32 = 30;

/// Takes the save's picture, and has `save` wait for it ([`waited`]).
fn picture_first(world: &mut bevy::prelude::World, save: Waits) {
    let since = world
        .get_resource::<crate::save_picture::Taken>()
        .map_or(0, |taken| taken.count);
    crate::save_picture::take_in(world);
    world.resource_mut::<Waiting>().0 = Some((save, since, 0));
}

/// Saves the save that waited, once its picture is taken or it has waited long enough.
fn waited(world: &mut bevy::prelude::World) {
    let Some((save, since, frames)) = world.resource_mut::<Waiting>().0.take() else {
        return;
    };
    let taken = world
        .get_resource::<crate::save_picture::Taken>()
        .map_or(0, |taken| taken.count);
    if taken <= since && frames < PICTURE_WAIT {
        world.resource_mut::<Waiting>().0 = Some((save, since, frames + 1));
        return;
    }
    match save {
        Waits::Quick => {
            if let Err(e) = save_in(world, QUICK_FILE, QUICK_SAVE) {
                warn!("saves: the quick save: {e:#}");
            }
        }
        Waits::Auto(number) => autosave_now(world, number),
    }
}

/// Forgets a load that did not get as far as its game (the menu again before its hero stood), so
/// that no later game takes its hero or its scripts.
fn forget(mut commands: Commands) {
    commands.remove_resource::<Restore>();
    commands.remove_resource::<Resuming>();
    commands.insert_resource(Held(false));
    commands.remove_resource::<WalkOn>();
    commands.remove_resource::<RideOn>();
    commands.remove_resource::<Resume>();
    commands.remove_resource::<drops::Again>();
}

/// The saves: a loaded game's hero and world put back, an exported hero's import, the quick save
/// and load and the scripts' autosave.
pub struct SavesPlugin;

impl Plugin for SavesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Asked>()
            .init_resource::<Held>()
            .init_resource::<AutoSaves>()
            .init_resource::<Waiting>()
            .add_systems(OnEnter(AppState::Menu), forget)
            .add_systems(
                Update,
                restore
                    .after(Step::HeroSpawn)
                    .run_if(in_state(AppState::InGame))
                    .run_if(resource_exists::<Restore>)
                    .run_if(|ready: Option<Res<crate::hero::Ready>>| ready.is_some_and(|r| r.0)),
            )
            .add_systems(
                Update,
                hold.run_if(in_state(AppState::InGame)).run_if(
                    |restore: Option<Res<Restore>>, resuming: Option<Res<Resuming>>| {
                        restore.is_some() || resuming.is_some()
                    },
                ),
            )
            .add_systems(
                Update,
                resume
                    .after(restore)
                    .run_if(in_state(AppState::InGame))
                    .run_if(resource_exists::<Resuming>),
            )
            .add_systems(
                Update,
                walk_on
                    .after(restore)
                    .run_if(in_state(AppState::InGame))
                    .run_if(resource_exists::<WalkOn>),
            )
            .add_systems(
                Update,
                ride_on
                    .after(restore)
                    .run_if(in_state(AppState::InGame))
                    .run_if(resource_exists::<RideOn>),
            )
            .add_systems(
                Update,
                import
                    .after(Step::HeroSpawn)
                    .run_if(in_state(AppState::InGame))
                    .run_if(resource_exists::<Import>)
                    .run_if(|ready: Option<Res<crate::hero::Ready>>| ready.is_some_and(|r| r.0)),
            )
            // Not while a load puts its game back, whose hero a save would not hold yet.
            .add_systems(
                Update,
                (quick_keys, quick, autosave, waited)
                    .chain()
                    .run_if(in_state(AppState::InGame))
                    .run_if(not(resource_exists::<Restore>))
                    .run_if(not(resource_exists::<Resuming>)),
            );
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn the_quick_keys_ask_for_a_save_but_under_a_stash_and_for_its_load() {
        use bevy::input::ButtonState;
        use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
        let mut app = App::new();
        app.add_plugins((bevy::MinimalPlugins, bevy::input::InputPlugin))
            .init_resource::<Asked>()
            .add_systems(Update, quick_keys);
        app.update();
        let press = |app: &mut App, keys: &[KeyCode]| {
            for state in [ButtonState::Pressed, ButtonState::Released] {
                for &key in keys {
                    app.world_mut().write_message(KeyboardInput {
                        key_code: key,
                        logical_key: Key::F9,
                        state,
                        text: None,
                        repeat: false,
                        window: Entity::PLACEHOLDER,
                    });
                }
                app.update();
            }
            app.world_mut().resource_mut::<Asked>().0.take()
        };
        assert_eq!(press(&mut app, &[KeyCode::F9]), Some(Quick::Save));
        assert_eq!(
            press(&mut app, &[KeyCode::ControlLeft, KeyCode::KeyZ]),
            Some(Quick::Save)
        );
        assert_eq!(press(&mut app, &[KeyCode::F8]), Some(Quick::Load));
        assert_eq!(
            press(&mut app, &[KeyCode::ControlRight, KeyCode::KeyX]),
            Some(Quick::Load)
        );
        // The town stash's window refuses the save, not the load.
        app.insert_resource(crate::town_stash::Opened(Some(Entity::PLACEHOLDER)));
        assert_eq!(press(&mut app, &[KeyCode::F9]), None);
        assert_eq!(press(&mut app, &[KeyCode::F8]), Some(Quick::Load));
        app.insert_resource(crate::town_stash::Opened(None));
        assert_eq!(press(&mut app, &[KeyCode::F9]), Some(Quick::Save));
        // A dialogue's window refuses it too (#2878), and its close allows it again.
        app.insert_resource(crate::talk::Shown::of(Entity::PLACEHOLDER));
        assert_eq!(press(&mut app, &[KeyCode::F9]), None);
        assert_eq!(press(&mut app, &[KeyCode::F8]), Some(Quick::Load));
        app.world_mut().remove_resource::<crate::talk::Shown>();
        assert_eq!(press(&mut app, &[KeyCode::F9]), Some(Quick::Save));
    }

    #[test]
    fn an_autosave_takes_the_first_slot_from_1_that_no_save_holds() {
        let dir = scratch("slots");
        assert_eq!(free_slot(&dir), "GAME01");
        for stem in ["GAME00", "GAME01", "GAME03"] {
            fs::write(dir.join(format!("{stem}.{EXTENSION}")), b"").unwrap();
        }
        assert_eq!(free_slot(&dir), "GAME02");
        fs::write(dir.join(format!("GAME02.{EXTENSION}")), b"").unwrap();
        assert_eq!(free_slot(&dir), "GAME04");
        fs::remove_dir_all(&dir).unwrap();
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "opensacred-game-saves-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn header(name: &str, saved: u64) -> Header {
        Header {
            version: VERSION,
            name: name.to_owned(),
            class: Class::Dwarf,
            hero: "Dwarf".to_owned(),
            difficulty: Difficulty::Silver,
            level: 7,
            clock: 8.0 * 3600.0,
            saved,
            tree: Some("addon-type_npc_zwerg".to_owned()),
        }
    }

    pub(crate) fn hero() -> Hero {
        Hero {
            at: (1234.5, 6789.0),
            floor: 1,
            heading: 135.0,
            experience: Experience::default(),
            base: opensacred_rules::fight::Base::default(),
            health: 99,
            effects: Effects::default(),
            arts: Arts::default(),
            slots: ArtSlots::default(),
            selected: Some(1078),
            combos: HeroCombos::default(),
            pack: Pack::default().kept(),
            sets: WeaponSets::default(),
            home: Some(Teleport {
                cell: (3236, 2511),
                layer: 1,
                heading: None,
            }),
            tally: Tally::default(),
            kills: Kills::default(),
            history: History::default(),
            wins: Wins([0, 1, 0, 0, 0]),
            runes_given: 3,
            quests: opensacred_rules::rewards::QuestCounts {
                quests: 12,
                since_piece: 5,
            },
            survival: 125.0,
            clock: Clock::at(21, 30),
            queue: crate::npcs::SavedQueue::default(),
            chain: 1.5,
            marks: crate::fight::KnockMarks::SECOND,
            zoom: 1.5,
            walk: Some(Walk {
                to: (1300.0, 0.0, 6800.0),
                floor: Some(0),
            }),
            windows: Windows {
                book: false,
                inventory: true,
            },
        }
    }

    #[test]
    fn a_load_gives_back_the_day_bank_of_the_art_slots() {
        let mut night = ArtSlots::carried(
            [Some(1022), None, None, None, None],
            [None, Some(1023), None, None, None],
            1,
        );
        night.turn(true);
        let saved = Hero {
            slots: night,
            ..hero()
        };
        let mut world = bevy::prelude::World::new();
        let entity = world.spawn_empty().id();
        put_character(&mut world, entity, &saved);
        let slots = world.resource::<ArtSlots>();
        assert_eq!(
            (slots.arts, slots.selected),
            ([Some(1022), None, None, None, None], 1)
        );
    }

    #[test]
    fn a_walk_goes_on_to_its_target_on_the_level_a_click_chose() {
        let heights = Heights::default();
        let target = Vec3::new(1300.0, 2.0, 6800.0);
        // No level chosen: whatever level the way reaches.
        let walk = Walk::of(target, None, Some(&heights));
        assert_eq!(walk.floor, None);
        assert_eq!(walk.order(&heights), (target, None));
        // The ground chosen: the ground's.
        let walk = Walk::of(target, Some(None), Some(&heights));
        assert_eq!(walk.floor, Some(0));
        assert_eq!(walk.order(&heights), (target, Some(None)));
        // A floor that no building holds at the target any more: whatever level the way reaches.
        let walk = Walk {
            floor: Some(2),
            ..walk
        };
        assert_eq!(walk.order(&heights), (target, None));
    }

    #[test]
    fn a_load_starts_at_the_saves_zoom_but_where_the_run_names_one() {
        let (header, hero) = (header("zoomed", 10), hero());
        let game = game_of(&header, &hero, crate::game::Shows::default());
        assert_eq!(game.zoom, Some(1.5));
        // The hero's sector, of its cell.
        let cell = crate::paths::cell_of(hero.at.0, hero.at.1);
        let side = i32::try_from(opensacred_data::world::SIDE).unwrap();
        let sector = |c: i32| u32::try_from(c.div_euclid(side)).unwrap();
        assert_eq!(game.sector, Some((sector(cell.0), sector(cell.1))));
        assert_eq!(game.tree.as_deref(), Some("addon-type_npc_zwerg"));
        // The run's `--zoom` holds over the save's.
        let shows = crate::game::Shows::of(&NewGame {
            zoom: Some(0.5),
            ..NewGame::default()
        });
        assert_eq!(game_of(&header, &hero, shows).zoom, Some(0.5));
    }

    /// The scripts of a game in the middle of its quests: a variable, a quest set up, a timer, a
    /// function done, the `rand()` on, two batches of the play's records of the world, a trigger
    /// opened and a quest's item taken.
    fn saved_scripts() -> Scripts {
        use opensacred_data::netscript::{Body, Operand, Record, Value};
        let mut state = opensacred_rules::quests::Scripts::default();
        state.variables.set("played", 3);
        state.quests.set_up(1202, &mut |_| true);
        state.timers.set(7, "zeit1202", 4320);
        // A cave's entrance that the lens draws (#2932).
        state.map_places.0.push(opensacred_rules::quests::MapPlace {
            cell: (3249, 2509),
            kind: 2,
        });
        let record = |kind, name: &str| Record {
            kind,
            body: Body::Operands(vec![Operand {
                tag: 0x01,
                value: Value::Str(name.to_owned()),
            }]),
        };
        Scripts {
            seed: 0x5EED,
            scripts: crate::scripts::Saved {
                state,
                done: vec![("start.luau".to_owned(), "Region3Init".to_owned())],
                rng: opensacred_rules::spawning::Rng::of(1, 2, 3),
                mode: crate::scripts::Mode {
                    anim: true,
                    cinema: false,
                },
            },
            npcs: crate::npcs::Saved {
                records: vec![
                    vec![record(1, "res:giver"), record(3, "chest")],
                    vec![record(5, "res:giver")],
                ],
                opened: ["gate".to_owned()].into(),
                taken: [12].into(),
                ..crate::npcs::Saved::default()
            },
        }
    }

    /// The world of a game played a while: gold in the town stash, the land about a cell explored
    /// and its sector entered, the map's pin and the weather (the creatures', the followers' and
    /// the drops' parts are `npcs`' and `drops`' tests').
    fn saved_world() -> World {
        let mut world = World::default();
        let gold = opensacred_rules::drops::gold_pile(250);
        world.stash.items.push((1, gold));
        let stamp = opensacred_rules::explored::Stamp::at(600, false);
        world.land.stamp((3200, 2500), stamp);
        world.sectors.enter((50, 39));
        world.pin = Some((Vec2::new(10.0, 20.0), Vec2::new(3200.0, 2500.0)));
        world.weather = Some(opensacred_rules::weather::Weather::default());
        world
    }

    #[test]
    fn a_save_reads_back_as_written_and_lists_the_newest_first() {
        let dir = scratch("round");
        let live = Live::default();
        let parts = (&hero(), &saved_scripts(), &saved_world(), &live);
        let picture = Picture([0x0F, 0xF0].repeat(128 * 96));
        write(
            &file(&dir, "first").unwrap(),
            (&header("first", 10), Some(&picture)),
            parts,
        )
        .unwrap();
        write(
            &file(&dir, "second one").unwrap(),
            (&header("second one", 20), None),
            parts,
        )
        .unwrap();
        // The picture after the header, read with it.
        let bytes = fs::read(file(&dir, "first").unwrap()).unwrap();
        assert_eq!(super::header(&bytes).unwrap().1, Some(picture));
        let (read_header, read_hero, read_scripts, read_world, read_live) =
            read(&file(&dir, "first").unwrap()).unwrap();
        assert_eq!(read_live, Live::default());
        assert_eq!(read_header, header("first", 10));
        assert_eq!(read_hero.at, (1234.5, 6789.0));
        assert_eq!(read_hero.heading, 135.0);
        assert_eq!((read_hero.floor, read_hero.health), (1, 99));
        assert_eq!(read_hero.selected, Some(1078));
        assert_eq!(read_hero.runes_given, 3);
        assert_eq!(read_hero.chain, 1.5);
        assert_eq!(read_hero.marks, crate::fight::KnockMarks::SECOND);
        assert_eq!(read_hero.zoom, 1.5);
        assert_eq!(read_hero.walk, hero().walk);
        assert_eq!(read_hero.windows, hero().windows);
        assert_eq!(read_hero.quests.quests, 12);
        assert_eq!(read_hero.quests.since_piece, 5);
        assert_eq!(read_hero.home.map(|home| home.cell), Some((3236, 2511)));
        assert!((read_hero.clock.minutes() - (21.0 * 60.0 + 30.0)).abs() < 1e-6);
        // The scripts whole: their state, the functions done, the `rand()` and the placements'.
        assert_eq!(read_scripts, saved_scripts());
        // The world whole: the stash's gold, the land and the sectors, the pin, the weather.
        assert_eq!(read_world, saved_world());
        assert!(read_world.land.explored((3200, 2500)));
        assert!(read_world.sectors.entered((50, 39)));
        let names: Vec<String> = list(&dir).into_iter().map(|(name, _)| name).collect();
        assert_eq!(names, ["second one", "first"]);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The layout of every structure that a save holds, which postcard reads by its fields' order
    /// alone (#2306), traced from their types: each container's fields in order with their
    /// formats, each enum's variants, of which a digest is recorded beside the version.
    fn layout_digest() -> u64 {
        // The tracer follows only the first variant of an enum within another type: the enums
        // that it leaves incomplete are traced again, round by round, until none is left.
        let mut again: Vec<String> = Vec::new();
        loop {
            match traced(&again) {
                Ok(registry) => {
                    // FNV-1a over the registry's text, its containers in the order of their names.
                    return format!("{registry:?}")
                        .bytes()
                        .fold(0xCBF2_9CE4_8422_2325, |digest, byte| {
                            (digest ^ u64::from(byte)).wrapping_mul(0x0100_0000_01B3)
                        });
                }
                Err(serde_reflection::Error::MissingVariants(names)) => {
                    assert!(
                        names.iter().any(|name| !again.contains(name)),
                        "the enums {names:?} stay incomplete after {ROUNDS} rounds"
                    );
                    again.extend(names);
                    again.sort();
                    again.dedup();
                }
                Err(e) => panic!("tracing the saves' layout: {e}"),
            }
        }
    }

    /// The rounds of tracing that cover every variant of the enums within the saved structures,
    /// each round one more variant of each.
    const ROUNDS: usize = 256;

    /// The saved structures traced in [`ROUNDS`] rounds, the enums `again` traced again at each
    /// but the last, which tells those still incomplete.
    fn traced(again: &[String]) -> serde_reflection::Result<serde_reflection::Registry> {
        use serde_reflection::{Samples, Tracer, TracerConfig};
        let mut tracer = Tracer::new(TracerConfig::default());
        let samples = Samples::new();
        for round in 0..=ROUNDS {
            tracer.trace_type_once::<Header>(&samples)?;
            tracer.trace_type_once::<Option<Picture>>(&samples)?;
            tracer.trace_type_once::<Hero>(&samples)?;
            tracer.trace_type_once::<Scripts>(&samples)?;
            tracer.trace_type_once::<World>(&samples)?;
            tracer.trace_type_once::<Live>(&samples)?;
            if round < ROUNDS {
                for name in again {
                    tracer.check_incomplete_enum(name);
                }
            }
        }
        tracer.registry()
    }

    #[test]
    fn the_saves_layout_is_its_versions() {
        assert_eq!(
            (VERSION, layout_digest()),
            (82, 0xD7F1_3755_FA05_3481),
            "a saved structure's layout changed: raise VERSION and record the new digest beside \
             it (only the digest where nothing but a name or the tracer changed)"
        );
    }

    #[test]
    fn a_save_of_another_version_stands_in_the_slots_and_is_not_loaded() {
        let dir = scratch("stale");
        let live = Live::default();
        let parts = (&hero(), &saved_scripts(), &World::default(), &live);
        write(
            &file(&dir, "GAME01").unwrap(),
            (&header("now", 2), None),
            parts,
        )
        .unwrap();
        // An older save whose header reads as this layout, and one of whose bytes only the
        // version reads.
        let old = Header {
            version: VERSION - 1,
            ..header("old", 1)
        };
        write(&file(&dir, "GAME02").unwrap(), (&old, None), parts).unwrap();
        let mut bytes = postcard::to_allocvec(&(VERSION - 5)).unwrap();
        bytes.extend([0xFF; 3]);
        fs::write(file(&dir, "GAME03").unwrap(), bytes).unwrap();
        fs::write(file(&dir, "GAME04").unwrap(), []).unwrap();
        let (_, listed) = slots(&dir);
        let shown: Vec<(&str, &str, bool)> = listed
            .iter()
            .map(|slot| (slot.stem.as_str(), slot.header.name.as_str(), slot.stale))
            .collect();
        assert_eq!(
            shown,
            [
                ("GAME01", "now", false),
                ("GAME02", "old", true),
                ("GAME03", "GAME03", true)
            ]
        );
        assert!(listed[1].picture.is_none() && listed[2].header.saved > 0);
        assert!(read(&file(&dir, "GAME02").unwrap()).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_save_of_another_version_or_a_bad_name_is_refused() {
        let dir = scratch("version");
        let old = Header {
            version: VERSION + 1,
            ..header("old", 1)
        };
        let live = Live::default();
        let parts = (&hero(), &saved_scripts(), &World::default(), &live);
        write(&file(&dir, "old").unwrap(), (&old, None), parts).unwrap();
        let error = format!("{:#}", read(&file(&dir, "old").unwrap()).unwrap_err());
        assert!(error.contains("version"), "{error}");
        assert!(list(&dir).is_empty());
        // A save of the version before, of the layout before, is refused too.
        let before = Header {
            version: VERSION - 1,
            ..header("before", 2)
        };
        write(&file(&dir, "before").unwrap(), (&before, None), parts).unwrap();
        let error = format!("{:#}", read(&file(&dir, "before").unwrap()).unwrap_err());
        assert!(error.contains("version"), "{error}");
        for bad in ["", "../up", "a/b", "a.sav"] {
            assert!(file(&dir, bad).is_err(), "{bad}");
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}
