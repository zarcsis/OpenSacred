//! The creatures' drops (#415; docs/re/items.md, "Drops"): as an NPC dies, a hostile creature
//! killed by the living hero asks the rules for its drops (`opensacred_rules::drops`), and each
//! drop becomes its item's model with its texture (`items.pak +0x70`, `+0x08`), thrown from the
//! creature's place at the height 60 and flying by the rules (`opensacred_rules::drops::Flight`)
//! until it lies on the ground, where it stays: the pick-up comes with the inventory (M5), the
//! labels on Alt later (docs/decisions/2026-10-21-drops-in-game.md). The `state` command lists
//! the nearest drops, and the command `drop` makes the nearest hostile creature drop.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Result, anyhow};
use bevy::prelude::*;
use bevy::tasks::futures::check_ready;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use opensacred_data::tables::{ITEM_SETS, ItemSet, WEAPON_MODIFIERS, WeaponModifier};
use opensacred_rules::drops::{
    self, Catalogue, Dead, Death, Dropped, Flight, Hero, Kill, Kind, Rules,
};
use opensacred_rules::spawning::Rng;

use crate::frame::{Playing, Step, Unpaused};
use crate::game::{AppState, Data};
use crate::hero::Body;
use crate::npcs::{Dies, Retexture};
use crate::terrain::{Heights, Level};
use crate::{ai, crowd, fight, files, iso, models, npcs, sectors};

/// What the drops read: the balance's values, the item types to choose from, the templates that
/// make the items, and each item type's model and texture, relative to the asset directory.
#[derive(Resource)]
pub struct Tables {
    rules: Rules,
    catalogue: Catalogue,
    templates: fight::Templates,
    looks: HashMap<u32, (String, Option<String>)>,
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
        let catalogue = Catalogue::new(items, weapons, &sets, &templates);
        let models = npcs::model_files(assets)?;
        let textures = npcs::texture_files(assets)?;
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
        Ok(Self {
            rules: Rules::of(balance),
            catalogue,
            templates,
            looks,
        })
    }
}

/// The tables being read, at the start, off the main thread.
#[derive(Resource)]
struct Reading(Task<Result<Tables>>);

/// The rolls of the drops and their flights.
#[derive(Resource)]
pub(crate) struct Dice(Rng);

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

/// A drop in flight, the level it flies on, and whether it stays upright.
#[derive(Component)]
struct Flying {
    flight: Flight,
    level: Level,
    upright: bool,
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
    (health.current > 0).then(|| Hero {
        creature: base.id,
        level,
        skills: fight::skills(base, level, true),
    })
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

/// A creature's death as its drops take it: its type, class, level, worth and rank, the hero,
/// the time played, on bronze.
fn death(
    entity: Entity,
    mind: &ai::Mind,
    health: &fight::Health,
    rank: fight::Rank,
    worth: &fight::Worth,
    hero: Hero,
    played: f32,
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
            ..Kill::default()
        },
        difficulty: drops::Difficulty::Bronze,
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
            },
            DespawnOnExit(AppState::InGame),
        ));
        // A derived type shows its root's model and texture, the items table's entry it holds
        // (#596).
        let slot = tables.catalogue.slot(dropped.item).unwrap_or(dropped.item);
        if let Some((model, texture)) = tables.looks.get(&slot) {
            let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(model.clone()));
            entity.insert((WorldAssetRoot(scene), models::Ordered::default()));
            if let Some(texture) = texture {
                entity.insert(Retexture::load(assets, texture));
            }
        }
    }
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
    played: Res<Played>,
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
    heroes: Query<(&fight::Base, &fight::Health), (With<Body>, Without<Dies>)>,
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
    let hero = heroes
        .iter()
        .next()
        .and_then(|(base, health)| hero_of(base, health, experience.level));
    if let Some(why) = kept(mind, hero.as_ref()) {
        debug!("drops: {} drops nothing: {why}", died.entity);
        return;
    }
    let Some(hero) = hero else {
        return;
    };
    let death = death(died.entity, mind, health, rank.0, worth, hero, played.0);
    let made = drops::drops(
        &death,
        &tables.rules,
        &tables.catalogue,
        &tables.templates,
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
/// the height a creature stands at there; the gold piles and the runes stay upright.
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    heights: Option<Res<Heights>>,
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
            let at = transform.translation;
            debug!(
                "drops: {} lies at {:.0},{:.0},{:.0}",
                lying.dropped.item, at.x, at.y, at.z
            );
            commands.entity(entity).remove::<Flying>();
        }
    }
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
    .ok_or_else(|| format!("item {} is of no drop's kind", request.item))?;
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
    played: Res<Played>,
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
    let death = death(entity, mind, health, rank.0, worth, hero, played.0);
    let mut made = Vec::new();
    for _ in 0..times.0 {
        made.extend(drops::drops(
            &death,
            &tables.rules,
            &tables.catalogue,
            &tables.templates,
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
            skills: Vec::new(),
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
}
