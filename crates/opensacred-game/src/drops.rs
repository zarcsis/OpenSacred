//! The creatures' drops (#415; docs/re/items.md, "Drops"): as an NPC dies, a hostile creature
//! killed by the living hero asks the rules for its drops (`opensacred_rules::drops`), and each
//! drop becomes its item's model with its texture (`items.pak +0x70`, `+0x08`), thrown from the
//! creature's place at the height 60 and flying by the rules (`opensacred_rules::drops::Flight`)
//! until it lies on the ground, where it stays until the hero picks it up (`pickup`, #911) or the
//! game ends (docs/decisions/items.md, "Drops lie until the game ends"); the labels on Alt come
//! later. The `state` command lists the nearest drops, and the command `drop` makes the nearest
//! hostile creature drop.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Result, anyhow};
use bevy::prelude::*;
use bevy::tasks::futures::check_ready;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use opensacred_data::tables::{ITEM_SETS, ItemSet, RUNES, WEAPON_MODIFIERS, WeaponModifier};
use opensacred_rules::drops::{
    self, Catalogue, Dead, Death, Dropped, Flight, Hero, Kill, Kind, Rules,
};
use opensacred_rules::spawning::Rng;

use crate::frame::{Playing, Step, Unpaused};
use crate::game::{AppState, Data};
use crate::hero::Body;
use crate::lines::Texts;
use crate::npcs::{Dies, Retexture};
use crate::terrain::{Heights, Level};
use crate::{ai, crowd, fight, files, iso, models, npcs, sectors};

/// What the drops read: the balance's values, the item types to choose from, the templates that
/// make the items, each item type's model and texture, relative to the asset directory, its size
/// in the backpack's cells and its inventory's transform (#1268), and its category (#1269).
#[derive(Resource)]
pub struct Tables {
    rules: Rules,
    catalogue: Catalogue,
    templates: fight::Templates,
    looks: HashMap<u32, (String, Option<String>)>,
    sizes: HashMap<u32, (u8, u8)>,
    poses: HashMap<u32, Pose>,
    /// Where each type's model lies in a weapon set's slot of the bar, from the slot's corner
    /// (`0x434480`, #1347).
    in_slot: HashMap<u32, Vec2>,
    categories: HashMap<u32, u8>,
    /// The combat art that each rune teaches (`items.pak +0x6A`, `0x43AF10`; #1315).
    rune_arts: HashMap<u32, u16>,
    /// Each type's record's classes' bits (`+0x04`), weapon's kind (`+0x1E`) and values, which
    /// the use's test asks (#1288).
    records: HashMap<u32, (u32, u8, fight::ItemValues)>,
    /// Each type's price, its record's `+0x08` (`Weapon.pak +0x8C`), which a container's run of
    /// items takes off its value (`0x4A0C42`; #1381).
    prices: HashMap<u32, i64>,
    /// The types that glitter lying on the ground: their `items.pak` record's first word's bit
    /// 19, the byte `+0x02` bit 3 (`0x428460`; #1382, `glitter`).
    glittering: std::collections::HashSet<u32>,
    /// Each type's record's bytes `+0x14` to `+0x1B` as the file holds them: the level it asks,
    /// the strength, the dexterity, the charisma and the endurance it asks, its own level, and its
    /// skill and the skill's level, which the tooltip names (#1273).
    requirements: HashMap<u32, [u8; 8]>,
}

/// What a tooltip tells of an item's type (#1273): its record's word `+0x04` (its classes, its
/// quality and its difficulty), its weapon's kind, the level it asks and its own (`+0x14`,
/// `+0x19`, as the file holds them), its other requirements (the strength, the dexterity, the
/// charisma, the endurance, the skill and its level) and its price.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Record {
    pub word: u32,
    pub kind: u8,
    pub asked: u8,
    pub level: u8,
    pub requirements: [u8; 6],
    pub price: i64,
}

/// The bit of an `items.pak` record's byte `+0x02` of a type that glitters on the ground, its
/// first word's bit 19 (`0x428460`).
const GLITTERS: u8 = 0x08;

/// The price of a type without a record (`0x5D3D36`).
const DEFAULT_PRICE: i64 = 600;

/// The combat arts' runes' category (`items.pak +0x2E`), whose items the hero carries weigh
/// against a rune's drop (`0x5598B0`).
const RUNE_CATEGORY: u8 = 28;
/// `DROPRUNE`, a rune's drop's sound (docs/re/sound.md, "Items, gold and levels").
const DROP_RUNE: u32 = 7028;

/// How an item's model lies in the backpack's grid (`0x434270`, its `Weapon.pak` record's floats
/// `+0x00` to `+0x18`): a scale, three turns in radians and two offsets (docs/re/inventory.md,
/// "Drawing it").
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    pub scale: f32,
    pub turns: [f32; 3],
    pub offsets: [f32; 2],
}

/// A table of `tables/`.
fn table<T: serde::de::DeserializeOwned>(assets: &Path, file: &str) -> Result<T> {
    let path = assets.join("tables").join(file);
    let text =
        files::read_to_string(&path).map_err(|e| anyhow!("reading {}: {e}", path.display()))?;
    ron::from_str(&text).map_err(|e| anyhow!("parsing {}: {e}", path.display()))
}

impl Tables {
    /// The tables of `tables/` (the balance's values the original's where `balance.ron` lacks
    /// them) and the models and textures of the converted items.
    fn read(shared: &sectors::Shared) -> Result<Self> {
        let assets = &shared.assets;
        let items = shared.tables.items()?;
        let weapons = shared.tables.weapons()?;
        let sets: Vec<ItemSet> = table(assets, ITEM_SETS)?;
        let modifiers: Vec<WeaponModifier> = table(assets, WEAPON_MODIFIERS)?;
        let balance = shared.tables.balance().ok();
        let templates = fight::Templates::new(&modifiers, weapons);
        // The runes that the drops draw from (#1183), none from assets made before them.
        let runes: Vec<u32> = table(assets, RUNES).unwrap_or_else(|e| {
            warn!("drops: {e}; no runes drop");
            Vec::new()
        });
        let catalogue = Catalogue::new(items, weapons, &sets, &templates).with_runes(runes);
        let models = npcs::model_files(assets)?;
        let textures = npcs::texture_files(assets)?;
        let glittering = items
            .iter()
            .filter(|item| item.subclass & GLITTERS != 0)
            .map(|item| item.slot)
            .collect();
        let looks = items
            .iter()
            .filter_map(|item| {
                let model = models.get(&item.name.to_ascii_uppercase())?.clone();
                let texture = match item.words_08[0] {
                    0 => None,
                    number => textures.get(&number).cloned(),
                };
                Some((item.slot, (model, texture)))
            })
            .collect();
        // The items' sizes and poses in the backpack by their types' `Weapon.pak` records
        // (`0x428D60`, `0x434270`).
        let records: HashMap<u32, &opensacred_data::tables::Weapon> =
            weapons.iter().map(|weapon| (weapon.id, weapon)).collect();
        let sizes = items
            .iter()
            .map(|item| {
                let record = records
                    .get(&item.slot)
                    .map_or([0; 2], |weapon| [weapon.bytes_1c[0], weapon.bytes_1c[1]]);
                let category = item.words_14[6].to_le_bytes()[2];
                (
                    item.slot,
                    opensacred_rules::inventory::size(item.slot, record, category),
                )
            })
            .collect();
        let prices = records
            .iter()
            .map(|(&id, weapon)| {
                let word = |k: usize| i64::from(weapon.words_84.get(k).copied().unwrap_or(0));
                (id, word(2) | word(3) << 16)
            })
            .collect();
        let used = records
            .iter()
            .map(|(&id, weapon)| {
                let word = |k: usize| u32::from(weapon.words_84.get(k).copied().unwrap_or(0));
                (
                    id,
                    (
                        word(0) | word(1) << 16,
                        weapon.bytes_1c[2],
                        fight::ItemValues::of_record(&weapon.words_84),
                    ),
                )
            })
            .collect();
        let requirements = records
            .iter()
            .map(|(&id, weapon)| {
                let bytes = |k: usize| weapon.words_84.get(k).copied().unwrap_or(0).to_le_bytes();
                let [a, b] = bytes(8);
                let [c, d] = bytes(9);
                let [e, f] = bytes(10);
                let [g, h] = bytes(11);
                (id, [a, b, c, d, e, f, g, h])
            })
            .collect();
        let rune_arts = items
            .iter()
            .filter_map(|item| {
                let art = u16::try_from(item.words_58[4] >> 16).ok()?;
                (art != 0).then_some((item.slot, art))
            })
            .collect();
        let categories = items
            .iter()
            .map(|item| (item.slot, item.words_14[6].to_le_bytes()[2]))
            .collect();
        let poses = records
            .iter()
            .map(|(&id, weapon)| {
                let [scale, a, b, c, first] = weapon.floats_00;
                (
                    id,
                    Pose {
                        scale,
                        turns: [a, b, c],
                        offsets: [first, weapon.float_18],
                    },
                )
            })
            .collect();
        // A model in a weapon set's slot (`0x434480`): the record's cells' half sizes (`+0x1C`,
        // `+0x1D`, 16 pixels a cell) less its signed bytes `+0x14` and `+0x15`, and 2 and 5.
        let in_slot = records
            .iter()
            .map(|(&id, weapon)| {
                let [width, height, ..] = weapon.bytes_1c;
                let [x, y, ..] = weapon.bytes_14.map(|byte| f32::from(byte.cast_signed()));
                (
                    id,
                    Vec2::new(
                        f32::from(width) * 16.0 - x + 2.0,
                        f32::from(height) * 16.0 - y + 5.0,
                    ),
                )
            })
            .collect();
        Ok(Self {
            rules: Rules::of(balance),
            catalogue,
            templates,
            looks,
            sizes,
            poses,
            in_slot,
            categories,
            rune_arts,
            records: used,
            requirements,
            prices,
            glittering,
        })
    }

    /// The key of the name that the item maker gave `dropped`, where it made it, of the names
    /// that `texts` holds a text for (`opensacred_rules::drops::name`, `0x5C3130`; #1424): the
    /// words of its quality, its element, its class and its bonuses, else its family's.
    pub fn made_name(&self, texts: &Texts, dropped: &Dropped) -> Option<String> {
        drops::name(
            dropped,
            (&self.rules, &self.catalogue, &self.templates),
            |key| texts.named(key).is_some_and(|text| !text.is_empty()),
        )
    }

    /// Whether an item of the type `item` glitters lying on the ground, by its derived type's
    /// root's record.
    pub fn glitters(&self, item: u32) -> bool {
        let slot = self.catalogue.slot(item).unwrap_or(item);
        self.glittering.contains(&item) || self.glittering.contains(&slot)
    }

    /// The values of `dropped` that its wearer's fight takes (#1290): its maker's where it gave it
    /// some, else its type's record's, of its type's category.
    pub fn values(&self, dropped: &Dropped) -> fight::ItemValues {
        let slot = self.catalogue.slot(dropped.item).unwrap_or(dropped.item);
        let record = self
            .records
            .get(&dropped.item)
            .or_else(|| self.records.get(&slot))
            .map(|&(_, _, values)| values)
            .unwrap_or_default();
        fight::ItemValues {
            class: self.category(dropped.item),
            ..dropped.values.unwrap_or(record)
        }
    }

    /// What `dropped` asks of the hero that would use it (#1288): its type's record's classes'
    /// bits and weapon's kind, its category, and the level asked of its own level (`+0x14` of
    /// `+0x19`, #1303), of its values where its maker gave it some, else of its record's.
    pub fn asks(
        &self,
        dropped: &Dropped,
        rules: &opensacred_rules::fight::Rules,
    ) -> opensacred_rules::equipment::Asks {
        let slot = self.catalogue.slot(dropped.item).unwrap_or(dropped.item);
        let (classes, kind, values) = self
            .records
            .get(&dropped.item)
            .or_else(|| self.records.get(&slot))
            .cloned()
            .unwrap_or_default();
        let values = dropped.values.unwrap_or(values);
        opensacred_rules::equipment::Asks {
            item: dropped.item,
            category: self.category(dropped.item),
            kind,
            classes,
            level: opensacred_rules::equipment::asked_level(opensacred_rules::fight::item_level(
                &values, rules,
            )),
        }
    }

    /// What a tooltip tells of the type `item`'s record, of its derived type's root (#1273).
    pub(crate) fn record(&self, item: u32) -> Record {
        let slot = self.catalogue.slot(item).unwrap_or(item);
        let of = |item: u32| {
            let (word, kind, _) = self.records.get(&item)?;
            let [
                asked,
                strength,
                dexterity,
                charisma,
                endurance,
                level,
                skill,
                skill_level,
            ] = self.requirements.get(&item).copied().unwrap_or_default();
            Some(Record {
                word: *word,
                kind: *kind,
                asked,
                level,
                requirements: [strength, dexterity, charisma, endurance, skill, skill_level],
                price: self.prices.get(&item).copied().unwrap_or_default(),
            })
        };
        of(item).or_else(|| of(slot)).unwrap_or_default()
    }

    /// The category of an item of the type `item` (`items.pak +0x2E`, `0x426220`), of its derived
    /// type's root; 0 for a type of none.
    pub fn category(&self, item: u32) -> u8 {
        let slot = self.catalogue.slot(item).unwrap_or(item);
        self.categories
            .get(&item)
            .or_else(|| self.categories.get(&slot))
            .copied()
            .unwrap_or(0)
    }

    /// The combat art that a rune of the type `item` teaches, of its derived type's root
    /// (`0x43AF10`); none for a type of no art.
    pub fn rune_art(&self, item: u32) -> Option<u16> {
        let slot = self.catalogue.slot(item).unwrap_or(item);
        self.rune_arts
            .get(&item)
            .or_else(|| self.rune_arts.get(&slot))
            .copied()
    }

    /// The size in the backpack's cells of an item of the type `item` (`0x428D60`), of its
    /// derived type's root (#596); 1 × 1 for a type of none.
    pub fn size(&self, item: u32) -> (u8, u8) {
        let slot = self.catalogue.slot(item).unwrap_or(item);
        self.sizes
            .get(&item)
            .or_else(|| self.sizes.get(&slot))
            .copied()
            .unwrap_or((1, 1))
    }

    /// The weapon's kind of an item of the type `item` (`Weapon.pak +0x1E`), by which its hands go
    /// (`0x5CEC00`, #1349), of its derived type's root; 0 for a type of no record.
    pub fn weapon_kind(&self, item: u32) -> u8 {
        let slot = self.catalogue.slot(item).unwrap_or(item);
        self.records
            .get(&item)
            .or_else(|| self.records.get(&slot))
            .map_or(0, |&(_, kind, _)| kind)
    }

    /// Where the model of an item of the type `item` lies in a weapon set's slot of the bar, from
    /// the slot's corner (`0x434480`), of its derived type's root; the corner for a type of no
    /// record.
    pub fn in_slot(&self, item: u32) -> Vec2 {
        let slot = self.catalogue.slot(item).unwrap_or(item);
        self.in_slot
            .get(&item)
            .or_else(|| self.in_slot.get(&slot))
            .copied()
            .unwrap_or(Vec2::ZERO)
    }

    /// The model, the texture and the backpack's pose of an item of the type `item`, of its
    /// derived type's root.
    pub fn look(&self, item: u32) -> Option<(&str, Option<&str>, Pose)> {
        let slot = self.catalogue.slot(item).unwrap_or(item);
        let (model, texture) = self.looks.get(&slot)?;
        let pose = self
            .poses
            .get(&item)
            .or_else(|| self.poses.get(&slot))
            .copied()
            .unwrap_or(Pose {
                scale: 1.0,
                ..Pose::default()
            });
        Some((model.as_str(), texture.as_deref(), pose))
    }

    /// The item types to choose from, which the creatures' weapons are chosen anew from too
    /// (`npcs`, #1217).
    pub(crate) fn catalogue(&self) -> &Catalogue {
        &self.catalogue
    }

    /// The price of an item of type `item`, its record's (`+0x08`); 600, the default of
    /// `0x5D3AB0`, for a type without a record.
    pub(crate) fn price(&self, item: u32) -> i64 {
        self.prices.get(&item).copied().unwrap_or(DEFAULT_PRICE)
    }

    /// The drops' balance's values, which the quests' rewards take too (`rewards`, #1386).
    pub(crate) fn rules(&self) -> &Rules {
        &self.rules
    }
}

/// The tables being read, at the start, off the main thread.
#[derive(Resource)]
struct Reading(Task<Result<Tables>>);

/// The rolls of the drops and their flights.
#[derive(Resource)]
pub(crate) struct Dice(pub(crate) Rng);

/// The creature that made the last jewel, which makes no other until another makes one (#610).
#[derive(Resource, Default)]
struct LastJewel(drops::LastJewel);

/// The time the game has been played, in seconds, which raises a kill's bonus and the level of a
/// script's creature (**hypothesis** for the original's counter `+0x570C`).
#[derive(Resource, Default)]
pub(crate) struct Played(f32);

impl Played {
    /// The whole minutes played, as the original counts them for a script's creature's level
    /// (`0x424650`, its seconds over 60, at `0x48463A`).
    pub(crate) fn minutes(&self) -> u32 {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "whole minutes, positive, far below the integers' range"
        )]
        let minutes = (self.0 / 60.0).floor().max(0.0) as u32;
        minutes
    }
}

/// A drop: what it is, and whether it rests on the ground.
#[derive(Component, Clone, Copy, Debug)]
pub struct Lying {
    pub dropped: Dropped,
    pub resting: bool,
}

/// A drop in flight, the level it flies on, whether it stays upright, and whether it is the
/// hero's marked item (`+0xB0` bit `0x8000000`, which a container's throw `0x5C9BA0` sets),
/// which lands within the hero's reach.
#[derive(Component)]
struct Flying {
    flight: Flight,
    level: Level,
    upright: bool,
    marked: bool,
}

/// World units of the height of `height` units of the models' height (`terrain`).
fn up(height: f32) -> f32 {
    iso::height(height * iso::PIXELS_PER_MODEL_HEIGHT)
}

/// The world point of a flight's point.
fn world([x, y, z]: [f32; 3]) -> Vec3 {
    Vec3::new(x, up(z), y)
}

/// The turn of a drop's model by its flight's angles about the original's axes: x, the ground's
/// y (the world's z) and the height (the world's y).
fn turn([x, y, z]: [f32; 3]) -> Quat {
    Quat::from_euler(EulerRot::YZX, z, y, x)
}

/// What the hero is to the drops, if it lives: its type, its level and its skills.
fn hero_of(base: &fight::Base, health: &fight::Health, level: u32) -> Option<Hero> {
    (health.current > 0).then(|| hero(base, level))
}

/// What the hero of the values `base` at the level `level` is to the choice of an item's type:
/// its type, its level and its skills.
pub(crate) fn hero(base: &fight::Base, level: u32) -> Hero {
    Hero {
        creature: base.id,
        level,
        skills: fight::skills(base, level, true),
        ..Hero::default()
    }
}

/// Gives `hero` the runes it carries in `pack` and its arts' levels of `arts`, by which a rune's
/// chance goes (`0x5598B0`, `0x5595A0`).
pub(crate) fn carrying(
    hero: &mut Hero,
    pack: &crate::pickup::Pack,
    arts: Option<&crate::arts::HeroArts>,
    tables: &Tables,
) {
    let runes = pack
        .items
        .iter()
        .filter(|(_, dropped)| tables.category(dropped.item) == RUNE_CATEGORY)
        .count();
    hero.runes = u32::try_from(runes).unwrap_or(u32::MAX);
    hero.arts = arts.map_or(0, |arts| arts.arts.levels());
}

/// Throws `dropped` from a container at `from` to `to` on `level` (`0x5C9BA0`), along the arc of
/// [`Flight::to_point`], marked as the hero's: where it lands out of the hero's reach it is
/// moved to the hero ([`fly`]).
pub(crate) fn throw_to(
    commands: &mut Commands,
    (assets, tables): (&AssetServer, &Tables),
    (from, to, level): (Vec3, Vec3, Level),
    dropped: Dropped,
) {
    let flight = Flight::to_point([from.x, from.z], [to.x, to.z]);
    lay(commands, (assets, tables), (flight, level, true), dropped);
}

/// Makes the item of a container's contents, `item` at `level` (`0x5D57C0` as `FillChest` calls
/// it, #1381), or gold of `amount` in the pile `item`; none of a type the table does not hold. Its
/// quality key is a death's pass's of no bonus, held at 5 or above, the item maker's at most 11
/// (**hypothesis**: the original rolls it by the hero's luck and the time played, not read).
pub(crate) fn contained(
    tables: &Tables,
    dice: &mut Dice,
    (item, level, amount): (u32, u32, u32),
) -> Option<Dropped> {
    let key = tables
        .rules
        .key(0, tables.rules.spread(1.0), &mut dice.0)
        .max(CONTAINED_LEAST_KEY);
    drops::thrown(
        &tables.catalogue,
        &tables.templates,
        (item, level, u8::try_from(key).unwrap_or(17), amount),
        &mut dice.0,
    )
}

/// The least quality key of a container's items, the item maker's quality 11 (`0x5D57C0`).
const CONTAINED_LEAST_KEY: u32 = 5;

/// Lays a quest's reward, an item of type `item`, at `at` on `level` (`0x4AC940`, `0x45FC60`:
/// made beside the hero), rising from 60 over the ground as a dropped item ([`Flight::dropped`]),
/// its values made at `made_at` of the item maker's key `quality` (`16 − key`, 0 for none); the
/// line of what it laid, none for a type the table does not hold.
pub(crate) fn lay_reward(
    commands: &mut Commands,
    (assets, tables, dice): (&AssetServer, &Tables, &mut Dice),
    (at, level): (Vec3, Level),
    (item, made_at, quality): (u32, u32, i64),
) -> Option<String> {
    let key = match u8::try_from(quality) {
        Ok(quality @ 1..=14) => 16 - quality,
        _ => 16,
    };
    let dropped = drops::thrown(
        &tables.catalogue,
        &tables.templates,
        (item, made_at, key, 1),
        &mut dice.0,
    )?;
    let line = listed(&[dropped]);
    let flight = Flight::dropped(at.x, at.z, &mut dice.0);
    lay(commands, (assets, tables), (flight, level, false), dropped);
    Some(line)
}

/// Why a creature drops nothing as it dies (docs/re/items.md, "Who drops"), or `None` when it
/// drops: not hostile to the hero, of the types 71 or 75; the hero dead.
fn kept(mind: &ai::Mind, hero: Option<&Hero>) -> Option<&'static str> {
    if hero.is_none() {
        Some("the hero is dead")
    } else if !mind.hostile() {
        Some("not hostile to the hero")
    } else if drops::never_drops(mind.creature) {
        Some("of a type that drops nothing")
    } else {
        None
    }
}

/// The drops' `s` of the hero's kills: the record 55's value 1 at the level of the greed's effect
/// that holds on it, 0 without (`0x53464E`, `opensacred_rules::spells::treasure`).
fn treasure(
    effects: Option<&crate::effects::Effects>,
    arts: Option<&crate::arts::HeroArts>,
) -> u32 {
    let level = effects.and_then(|held| {
        held.timed()
            .iter()
            .find(|timed| timed.effect.id == opensacred_rules::spells::GREEDY)
            .map(|timed| u32::try_from(timed.effect.whole).unwrap_or(0))
    });
    let greed = arts.and_then(|arts| {
        let spells = arts.tables.spells.iter();
        spells
            .clone()
            .find(|spell| spell.id == opensacred_rules::spells::GREED)
    });
    match (level, greed) {
        (Some(level), Some(greed)) => opensacred_rules::spells::treasure(greed, level),
        _ => 0,
    }
}

/// A creature's death as its drops take it: its type, class, level, worth and rank, the hero,
/// the time played and its greed's `s`, on the game's difficulty.
fn death(
    entity: Entity,
    mind: &ai::Mind,
    health: &fight::Health,
    rank: fight::Rank,
    worth: &fight::Worth,
    hero: Hero,
    (played, treasure, difficulty): (f32, u32, drops::Difficulty),
) -> Death {
    Death {
        dead: Dead {
            id: entity.to_bits(),
            creature: mind.creature,
            class: mind.class,
            level: health.level,
            worth: worth.0[1],
            rank,
        },
        hero,
        kill: Kill {
            minutes: played / 60.0,
            treasure,
            ..Kill::default()
        },
        difficulty,
    }
}

/// Throws the drops of a creature at `at`, on `level`: each its model, if it was converted, with
/// its texture, at the throw's height and turn.
fn throw(
    commands: &mut Commands,
    assets: &AssetServer,
    tables: &Tables,
    dice: &mut Rng,
    (at, level): (Vec3, Level),
    made: Vec<Dropped>,
) {
    for dropped in made {
        let flight = Flight::thrown(at.x, at.z, dice);
        lay(commands, (assets, tables), (flight, level, false), dropped);
    }
}

/// Puts `dropped` into the world on its `flight` on `level`, marked as the hero's or not, its
/// model, if it was converted, with its texture.
fn lay(
    commands: &mut Commands,
    (assets, tables): (&AssetServer, &Tables),
    (flight, level, marked): (Flight, Level, bool),
    dropped: Dropped,
) {
    let upright = drops::upright(dropped.item);
    let mut entity = commands.spawn((
        Transform::from_translation(world(flight.at)).with_scale(Vec3::splat(iso::MODEL_SCALE)),
        Lying {
            dropped,
            resting: false,
        },
        Flying {
            flight,
            level,
            upright,
            marked,
        },
        DespawnOnExit(AppState::InGame),
    ));
    // A derived type shows its root's model and texture, the items table's entry it holds
    // (#596).
    let slot = tables.catalogue.slot(dropped.item).unwrap_or(dropped.item);
    if let Some((model, texture)) = tables.looks.get(&slot) {
        let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(model.clone()));
        // An item, of a `Weapon.pak` record, which the original draws under the surface of a
        // liquid cell (`0x434070`).
        entity.insert((
            WorldAssetRoot(scene),
            models::Ordered::default(),
            models::Sinks,
        ));
        if let Some(texture) = texture {
            entity.insert(Retexture::load(assets, texture));
        }
    }
}

/// Drops `dropped` off the cursor at the centre of the cell `cell` on `level`
/// (`cEngine::dropItem`, `0x611620`): it rises from 60 over the ground on [`Flight::dropped`].
pub(crate) fn drop_at(
    commands: &mut Commands,
    (assets, tables, dice): (&AssetServer, &Tables, &mut Dice),
    (cell, level): ((i32, i32), Level),
    dropped: Dropped,
) {
    let at = crate::paths::centre(cell);
    let flight = Flight::dropped(at.x, at.y, &mut dice.0);
    lay(commands, (assets, tables), (flight, level, false), dropped);
}

/// The line of the log of a creature's drops.
fn listed(made: &[Dropped]) -> String {
    made.iter()
        .map(|d| match d.kind {
            Kind::Gold => format!("gold {} x{}", d.item, d.amount),
            kind => format!("{kind:?} {} level {} key {}", d.item, d.level, d.key).to_lowercase(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Makes the drops of each NPC that dies (`0x533BA8`, the AI's state 9), before its dying clip:
/// a hostile creature killed by the living hero drops, one that another creature killed nothing.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "an observer takes its resources and queries as arguments, its queries with their filters"
)]
fn dies(
    died: On<Add<Dies>>,
    mut commands: Commands,
    tables: Option<Res<Tables>>,
    mut dice: ResMut<Dice>,
    mut last: ResMut<LastJewel>,
    (played, difficulty): (Res<Played>, Option<Res<crate::game::Difficulty>>),
    experience: Res<fight::Experience>,
    assets: Res<AssetServer>,
    creatures: Query<(
        &ai::Mind,
        &fight::Health,
        &fight::Ranked,
        &fight::Worth,
        &Transform,
        Option<&crowd::InTheWay>,
        &Dies,
    )>,
    heroes: Query<
        (
            &fight::Base,
            &fight::Health,
            Option<&crate::effects::Effects>,
        ),
        (With<Body>, Without<Dies>),
    >,
    hero_arts: Option<Res<crate::arts::HeroArts>>,
    scripted: Option<Res<crate::scripts::ScriptDrops>>,
    (pack, mut sounds): (Res<crate::pickup::Pack>, MessageWriter<crate::sfx::Play>),
) {
    let Ok((mind, health, rank, worth, transform, way, dies)) = creatures.get(died.entity) else {
        return;
    };
    let Some(tables) = tables else {
        return;
    };
    // Killed by a creature that is no player's nor in the hero's list, it drops nothing (#630).
    if dies.by_creature {
        debug!("drops: {} drops nothing: a creature killed it", died.entity);
        return;
    }
    // An undead that rises again drops nothing at this death (`0x533C7F`, #1199).
    if dies.rises {
        debug!("drops: {} drops nothing: it rises again", died.entity);
        return;
    }
    let treasure = heroes
        .iter()
        .next()
        .map_or(0, |(_, _, effects)| treasure(effects, hero_arts.as_deref()));
    let hero = heroes
        .iter()
        .next()
        .and_then(|(base, health, _)| hero_of(base, health, experience.level));
    if let Some(why) = kept(mind, hero.as_ref()) {
        debug!("drops: {} drops nothing: {why}", died.entity);
        return;
    }
    let Some(mut hero) = hero else {
        return;
    };
    carrying(&mut hero, &pack, hero_arts.as_deref(), &tables);
    let death = death(
        died.entity,
        mind,
        health,
        rank.0,
        worth,
        hero,
        (
            played.0,
            treasure,
            difficulty.map_or_else(Default::default, |difficulty| difficulty.0),
        ),
    );
    // The scripts' drops of its type (`SetDrop`, #930).
    let scripted: Vec<(u32, u32)> = scripted
        .map(|scripted| scripted.0.of(mind.creature).collect())
        .unwrap_or_default();
    let made = drops::drops(
        &death,
        (&tables.rules, &tables.catalogue, &tables.templates),
        &scripted,
        &mut last.0,
        &mut dice.0,
    );
    if made.is_empty() {
        debug!(
            "drops: {} of type {} and level {} drops nothing",
            died.entity, mind.creature, health.level
        );
    } else {
        info!(
            "drops: {} of type {} and level {} drops {}",
            died.entity,
            mind.creature,
            health.level,
            listed(&made)
        );
    }
    // A rune's drop sounds at the creature (`0x559BF0` at `0x55A4BD`; #1167).
    for _ in made.iter().filter(|drop| drop.kind == Kind::Rune) {
        sounds.write(crate::sfx::Play {
            sound: crate::sfx::Of::Id(DROP_RUNE),
            at: Some(died.entity),
            rate: (0.5, crate::sfx::Roll::None),
        });
    }
    let level = way.and_then(|way| way.level);
    throw(
        &mut commands,
        &assets,
        &tables,
        &mut dice.0,
        (transform.translation, level),
        made,
    );
}

/// Flies each drop a frame on (`0x44DFE0`): it rises and falls about where it was thrown, its
/// velocity along the ground turning back at every frame as the original's (#478), and lies at
/// the height a creature stands at there; the gold piles and the runes stay upright. The hero's
/// marked item that lands out of its reach ([`out_of_reach`]) is moved to the hero's point
/// (`0x44F4D0`: `0x5FC8B0`, then `0x5FCCB0` with the hero's place), so that the hero can pick it
/// up; the original's network game leaves it where another player within 2,000 on its level
/// reaches it.
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    heights: Option<Res<Heights>>,
    heroes: Query<(&Transform, &Body), Without<Flying>>,
    mut flying: Query<(Entity, &mut Flying, &mut Lying, &mut Transform)>,
) {
    let Some(heights) = heights else {
        return;
    };
    let dt = time.delta_secs();
    for (entity, mut flying, mut lying, mut transform) in &mut flying {
        let level = flying.level;
        let ground = |x: f32, y: f32| {
            heights
                .standing(x, y, level)
                .map_or(0.0, |height| height / up(1.0))
        };
        flying.flight.step(dt, ground);
        transform.translation = world(flying.flight.at);
        transform.rotation = if flying.upright {
            Quat::IDENTITY
        } else {
            turn(flying.flight.angles)
        };
        if flying.flight.resting {
            lying.resting = true;
            if flying.marked
                && let Some((hero, body)) = heroes.iter().next()
                && out_of_reach(hero.translation.xz(), transform.translation.xz(), |cell| {
                    heights.walkable_on(cell, body.level())
                })
            {
                let at = transform.translation;
                debug!(
                    "drops: {} lands at {:.0},{:.0} out of the hero's reach and is moved to it",
                    lying.dropped.item, at.x, at.z
                );
                transform.translation = hero.translation;
            }
            let at = transform.translation;
            debug!(
                "drops: {} lies at {:.0},{:.0},{:.0}",
                lying.dropped.item, at.x, at.y, at.z
            );
            commands.entity(entity).remove::<Flying>();
        }
    }
}

/// Whether an item at `item` lies out of the reach of the hero at `hero` (`0x44F4D0`): on a cell
/// that the hero may not stand on (`0x4EBCA0`), or with no straight line of walkable cells to it
/// from the hero (`0x4FCC80`).
fn out_of_reach(hero: Vec2, item: Vec2, walkable: impl Fn(crate::paths::Cell) -> bool) -> bool {
    !walkable(crate::paths::cell_of(item.x, item.y))
        || !crate::paths::straight(hero, item, walkable)
}

/// Counts the time played.
fn play(time: Res<Time>, mut played: ResMut<Played>) {
    played.0 += time.delta_secs();
}

/// Starts reading the tables.
fn begin(mut commands: Commands, data: Option<Res<Data>>) {
    let Some(data) = data else {
        return;
    };
    let shared = data.0.clone();
    let task = AsyncComputeTaskPool::get().spawn(async move { Tables::read(&shared) });
    commands.insert_resource(Reading(task));
}

/// Takes the tables once they are read; without them nothing drops.
fn read(mut commands: Commands, reading: Option<ResMut<Reading>>) {
    let Some(mut reading) = reading else {
        return;
    };
    let Some(result) = check_ready(&mut reading.0) else {
        return;
    };
    commands.remove_resource::<Reading>();
    match result {
        Ok(tables) => {
            info!(
                "drops: {} weapons, {} rings and {} amulets to drop",
                tables.catalogue.list(5).len(),
                tables.catalogue.list(8).len(),
                tables.catalogue.list(20).len()
            );
            commands.insert_resource(tables);
        }
        Err(e) => warn!("drops: nothing drops: {e:#}"),
    }
}

/// The drops nearest the hero, at most `n`: what each is, its item, its amount, how far it is,
/// and whether it rests (`state`).
pub fn nearest(
    hero: Vec3,
    drops: impl Iterator<Item = (Vec3, Lying)>,
    n: usize,
) -> Vec<(String, u32, u32, f32, bool)> {
    let mut found: Vec<_> = drops
        .map(|(at, lying)| {
            let d = lying.dropped;
            let kind = format!("{:?}", d.kind).to_lowercase();
            (
                kind,
                d.item,
                d.amount,
                (at - hero).xz().length(),
                lying.resting,
            )
        })
        .collect();
    found.sort_by(|a, b| a.3.total_cmp(&b.3));
    found.truncate(n);
    found
}

/// An item that the command `item` throws (#525).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Throw {
    /// Its type, an `items.pak` slot.
    pub item: u32,
    /// The point it is thrown from, on a level.
    pub at: (Vec3, Level),
    /// The level the item maker makes it at, the hero's by default; the pass's quality key; the
    /// pieces of gold, a pile's by its level by default.
    pub level: Option<u32>,
    pub key: u8,
    pub amount: Option<u32>,
}

/// Throws the item of `throw` as a death's drop is thrown (the command `item`, #525); a line of
/// what it threw.
pub fn throw_item(
    In(request): In<Throw>,
    mut commands: Commands,
    tables: Option<Res<Tables>>,
    mut dice: ResMut<Dice>,
    experience: Res<fight::Experience>,
    assets: Res<AssetServer>,
) -> Result<String, String> {
    let tables = tables.ok_or("the drops' tables are not read yet")?;
    let level = request.level.unwrap_or(experience.level);
    let amount = match request.amount {
        Some(amount) => amount,
        None => tables.rules.gold(level, &mut dice.0),
    };
    let dropped = drops::thrown(
        &tables.catalogue,
        &tables.templates,
        (request.item, level, request.key, amount),
        &mut dice.0,
    )
    .ok_or_else(|| format!("item {} is not in the items' table", request.item))?;
    let line = format!("thrown: {}", listed(&[dropped]));
    throw(
        &mut commands,
        &assets,
        &tables,
        &mut dice.0,
        request.at,
        vec![dropped],
    );
    Ok(line)
}

/// Makes the hostile creature nearest the hero drop `times` times as if the hero killed it,
/// leaving it alive (the command `drop`); a line of what dropped.
pub fn drop_nearest(world: &mut World, times: u32) -> Result<String, String> {
    world
        .run_system_cached_with(make_nearest, times)
        .map_err(|e| e.to_string())?
}

#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn make_nearest(
    times: In<u32>,
    mut commands: Commands,
    tables: Option<Res<Tables>>,
    mut dice: ResMut<Dice>,
    mut last: ResMut<LastJewel>,
    (played, difficulty): (Res<Played>, Option<Res<crate::game::Difficulty>>),
    experience: Res<fight::Experience>,
    assets: Res<AssetServer>,
    creatures: Query<
        (
            Entity,
            &ai::Mind,
            &fight::Health,
            &fight::Ranked,
            &fight::Worth,
            &Transform,
            Option<&crowd::InTheWay>,
        ),
        Without<Dies>,
    >,
    heroes: Query<(&fight::Base, &fight::Health, &Transform), (With<Body>, Without<Dies>)>,
) -> Result<String, String> {
    let tables = tables.ok_or("the drops' tables are not read")?;
    let (base, hero_health, at) = heroes.iter().next().ok_or("no hero")?;
    let hero = hero_of(base, hero_health, experience.level);
    let (entity, mind, health, rank, worth, transform, way) = creatures
        .iter()
        .filter(|(_, mind, ..)| mind.hostile())
        .min_by(|a, b| {
            let d = |t: &Transform| (t.translation - at.translation).length_squared();
            d(a.5).total_cmp(&d(b.5))
        })
        .ok_or("no hostile creature")?;
    if let Some(why) = kept(mind, hero.as_ref()) {
        return Ok(format!("creature {} drops nothing: {why}", mind.creature));
    }
    let hero = hero.ok_or("the hero is dead")?;
    let difficulty = difficulty.map_or_else(Default::default, |difficulty| difficulty.0);
    let death = death(
        entity,
        mind,
        health,
        rank.0,
        worth,
        hero,
        (played.0, 0, difficulty),
    );
    let mut made = Vec::new();
    for _ in 0..times.0 {
        made.extend(drops::drops(
            &death,
            (&tables.rules, &tables.catalogue, &tables.templates),
            &[],
            &mut last.0,
            &mut dice.0,
        ));
    }
    let line = format!(
        "creature {} of level {} drops {} in {} deaths{}{}",
        mind.creature,
        health.level,
        made.len(),
        times.0,
        if made.is_empty() { "" } else { ": " },
        listed(&made)
    );
    let level = way.and_then(|way| way.level);
    throw(
        &mut commands,
        &assets,
        &tables,
        &mut dice.0,
        (transform.translation, level),
        made,
    );
    Ok(line)
}

/// The drops.
pub struct DropsPlugin;

impl Plugin for DropsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Dice(Rng::of(0, 0, 415)))
            .init_resource::<LastJewel>()
            .init_resource::<Played>()
            .add_observer(dies)
            .add_systems(Startup, begin)
            .add_systems(Update, read)
            .add_systems(OnEnter(AppState::InGame), |mut played: ResMut<Played>| {
                played.0 = 0.0
            })
            .add_systems(
                Update,
                (play, fly)
                    .after(Step::Npcs)
                    .in_set(Playing)
                    .in_set(Unpaused),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mind(creature: u32, flags: u32) -> ai::Mind {
        ai::Mind::new(
            Vec3::ZERO,
            flags,
            (false, false),
            None,
            [70.0, 180.0],
            (creature, 3),
        )
    }

    #[test]
    fn a_hostile_creature_drops_for_a_living_hero() {
        let hero = Hero {
            creature: 1,
            level: 1,
            ..Hero::default()
        };
        assert_eq!(kept(&mind(76, 0x1), Some(&hero)), None);
        assert_eq!(
            kept(&mind(76, 0), Some(&hero)),
            Some("not hostile to the hero")
        );
        assert_eq!(
            kept(&mind(71, 0x1), Some(&hero)),
            Some("of a type that drops nothing")
        );
        assert_eq!(kept(&mind(76, 0x1), None), Some("the hero is dead"));
        // A dead hero is no hero to the drops.
        let base = fight::Base::default();
        let dead = fight::Health {
            current: 0,
            most: 100,
            level: 1,
        };
        assert!(hero_of(&base, &dead, 1).is_none());
    }

    #[test]
    fn a_drops_height_and_the_nearest_drops() {
        // 60 units of the models' height are about 94 world units.
        assert!((world([1.0, 2.0, 60.0]) - Vec3::new(1.0, 94.06, 2.0)).length() < 0.1);
        let lying = |item, amount| Lying {
            dropped: Dropped {
                kind: Kind::Gold,
                item,
                level: 0,
                key: 16,
                quality: 0,
                amount,
                values: None,
                named: None,
            },
            resting: true,
        };
        let near = nearest(
            Vec3::ZERO,
            [
                (Vec3::new(100.0, 5.0, 0.0), lying(5133, 40)),
                (Vec3::new(0.0, 0.0, 30.0), lying(5132, 17)),
            ]
            .into_iter(),
            1,
        );
        assert_eq!(near, [("gold".to_owned(), 5132, 17, 30.0, true)]);
    }

    #[test]
    fn the_heros_marked_item_lies_out_of_its_reach_on_or_beyond_a_closed_cell() {
        // A wall of closed cells across the cells' column 5.
        let walkable = |(x, _): crate::paths::Cell| x != 5;
        let hero = crate::paths::centre((2, 2));
        let at = |x, y| crate::paths::centre((x, y));
        assert!(!out_of_reach(hero, at(3, 4), walkable));
        assert!(out_of_reach(hero, at(5, 3), walkable));
        assert!(out_of_reach(hero, at(7, 2), walkable));
    }
}
