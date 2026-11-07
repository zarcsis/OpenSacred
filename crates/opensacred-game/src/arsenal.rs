//! The arms that the heroes' arts change. The Seraphim's 1018 (#754; docs/re/arts.md, "The
//! Seraphim's"): while her effect [`arts::BFG`] holds, her hands hold the item [`arts::BFG_ITEM`]
//! alone, with its values, its model and the clips of its class, her own items put away (the
//! adder `0x582D50`); as it ends the item goes and her own come back (the taker `0x586D80`; seen
//! 2026-10-24: her sword back in her hand and her values as before, 30 s after the art at the
//! level 1). Her 1019 (#2211; docs/re/arts.md, "The Seraphim's"): while her effect
//! [`arts::RAILGUN`] holds, her hands hold nothing, each weapon in them bursting into sparks of
//! its shape as it comes (the adder's case 17 makes an object 856 of the variant 2 at each and
//! takes them out of her hands, `0x556A30`; seen 2026-11-07: her sword gone 1.9 s after the art
//! and back as the effect's 30 s ended). The Gladiator's 1013 (#755; docs/re/arts.md, "The Gladiator's"): the item it hurls
//! leaves his hand (`0x556A30`), and his arms are those without it until he picks it up again
//! (`pickup`), the weapon going back into his empty hand (`0x54ACC0`).

use bevy::prelude::*;
use opensacred_rules::arts;
use opensacred_rules::drops::Dropped;
use opensacred_rules::fight::{combat_with, regeneration_with};

use crate::effects::Effects;
use crate::equipment::Worn;
use crate::fight::{self, Base, Combat, Experience, Holding, Regenerates, Rules};
use crate::forms::Form;
use crate::game::AppState;
use crate::hero::{self, Body, GameSpeed, Hero};

/// What the hero holds: its form for the class of the items in its hands (their clips, its
/// size), their models, their values for its fight, its weapons' sounds and its blade.
#[derive(Clone, Debug)]
pub struct Arms {
    pub form: Form,
    pub held: Vec<crate::npcs::Held>,
    pub holds: fight::Holds,
    pub armed: crate::sfx::Armed,
    pub blade: Option<crate::making::Blade>,
}

/// The item that the Gladiator's 1013 hurls (#755): his arms without it, the item as it lies
/// once it falls, and its model and texture in flight.
#[derive(Clone, Debug)]
pub struct Hurled {
    pub arms: Arms,
    pub item: Dropped,
    pub held: crate::npcs::Held,
}

/// The hero's own arms, those of the Seraphim's effects [`arts::BFG`] and [`arts::RAILGUN`], the
/// Gladiator's without the item he hurls, and the effect whose arms it bears, none for its own.
#[derive(Resource)]
pub struct Arsenal {
    own: Arms,
    bfg: Option<Arms>,
    bolts: Option<Arms>,
    hurled: Option<Hurled>,
    borne: Option<u32>,
    thrown: bool,
}

impl Arsenal {
    pub fn new(own: Arms) -> Self {
        Self {
            own,
            bfg: None,
            bolts: None,
            hurled: None,
            borne: None,
            thrown: false,
        }
    }

    /// The Seraphim's, with the arms of her effect [`arts::BFG`].
    pub fn with_bfg(self, bfg: Arms) -> Self {
        Self {
            bfg: Some(bfg),
            ..self
        }
    }

    /// The Seraphim's, with her arms of empty hands, those of her effect [`arts::RAILGUN`].
    pub fn with_bolts(self, bolts: Arms) -> Self {
        Self {
            bolts: Some(bolts),
            ..self
        }
    }

    /// The Gladiator's, with the item his 1013 hurls.
    pub fn with_hurled(self, hurled: Hurled) -> Self {
        Self {
            hurled: Some(hurled),
            ..self
        }
    }

    /// The item that the hero hurls, while it holds it.
    pub fn hurls(&self) -> Option<&Hurled> {
        self.hurled.as_ref().filter(|_| !self.thrown)
    }

    /// The item that the hero hurls, as it lies once it falls.
    pub fn hurled_item(&self) -> Option<&Dropped> {
        self.hurled.as_ref().map(|hurled| &hurled.item)
    }

    /// Whether `item`, which the hero picks up, is the item it hurled, which goes back into its
    /// empty hand.
    pub fn takes_back(&self, item: &Dropped) -> bool {
        self.thrown
            && self
                .hurled
                .as_ref()
                .is_some_and(|hurled| hurled.item.item == item.item)
    }
}

/// The hero's item hurled out of its hand (`true`), or picked up into it again (`false`).
#[derive(Message, Clone, Copy, Debug)]
pub struct Throws(pub bool);

/// What makes the hero's arms anew as the items in its hands change, a weapon set chosen (#1348;
/// `0x6E1690`): its model, its motions' presets, its type and record, the clips' hit fractions and
/// points, the items its template wears besides those in its hands, and the items' records'
/// words, of which an item in its hands takes its values.
#[derive(Resource, Clone)]
pub struct Maker {
    pub assets: std::path::PathBuf,
    pub model: String,
    pub presets: [&'static str; 3],
    pub kind: u32,
    pub record: opensacred_data::tables::Creature,
    pub hits: std::collections::HashMap<String, [f32; 2]>,
    pub points: std::collections::HashMap<String, Vec<f32>>,
    pub worn: Vec<opensacred_data::heroes::Worn>,
    pub words: std::collections::HashMap<u32, Vec<u16>>,
}

impl Maker {
    /// The hero's arms with the items of `hands` in its hands, each of its type and of the item
    /// maker's values where it has some, else of its record's (#1349), its template's other items
    /// worn as ever; and the class of what it holds, by which it picks its clips (`0x546F80`).
    pub fn arms(
        &self,
        npcs: &crate::npcs::Npcs,
        hands: &[(u32, Option<fight::ItemValues>)],
    ) -> (u8, Arms) {
        let worn: Vec<opensacred_data::heroes::Worn> = self
            .worn
            .iter()
            .cloned()
            .chain(
                hands
                    .iter()
                    .map(|&(item, _)| opensacred_data::heroes::Worn {
                        id: 0,
                        item,
                        words: self.words.get(&item).cloned().unwrap_or_default(),
                    }),
            )
            .collect();
        let valued: Vec<Option<fight::ItemValues>> = self
            .worn
            .iter()
            .map(|_| None)
            .chain(hands.iter().map(|&(_, values)| values))
            .collect();
        let (class, held) = npcs.hero_gear(&worn);
        let clips = crate::game::clips_of(
            &self.assets,
            &self.model,
            self.presets,
            (self.kind, class),
            &self.hits,
            &self.points,
        );
        let arms = Arms {
            form: Form {
                creature: self.kind,
                model: self.model.clone(),
                clips,
                base: Base::of(&self.record),
                speeds: self.record.speed,
                size: opensacred_rules::minds::size(self.kind, class != 0),
                armed: true,
            },
            held,
            holds: npcs.hero_holds_valued(&worn, &valued),
            armed: npcs.hero_armed(&worn),
            blade: npcs.hero_blade(&worn),
        };
        (class, arms)
    }
}

/// The hero's arms by the items now in its hands, a weapon set chosen (#1348).
#[derive(Message, Clone, Debug)]
pub struct Rearm(pub Arms);

/// What of the hero its arms change.
type Wearer<'a> = (
    Entity,
    &'a Base,
    &'a Effects,
    Mut<'a, Combat>,
    Mut<'a, Regenerates>,
    Option<Mut<'a, hero::Size>>,
    Option<&'a Worn>,
    Option<&'a Holding>,
);

/// Gives the hero `arms`: its items' models and clips, the values of what it holds and of what
/// it wears in the equipment window (#1290), its size, and its values made anew with them, its
/// pace and speeds with them.
fn wear(
    arms: Arms,
    commands: &mut Commands,
    hero: &mut Hero,
    assets: &AssetServer,
    (experience, rules, speed): (&Experience, &Rules, &GameSpeed),
    (body, base, effects, mut combat, mut regenerates, size, worn, held): Wearer,
) {
    let mut holds = arms.holds;
    holds
        .items
        .extend(worn.into_iter().flat_map(|worn| worn.0.iter().cloned()));
    // The sets' bonuses kept, which `equipment::refit` makes anew as the hands' items change.
    holds.sets = held.map(|held| held.0.sets.clone()).unwrap_or_default();
    debug!("arsenal: the hero holds {:?}", arms.held);
    hero.held = arms.held;
    hero.armed = arms.armed;
    hero.blade = arms.blade;
    // The blade of the hurled blades, which the hero's body carries (`hero::spawn`).
    match arms.blade {
        Some(blade) => {
            commands.entity(body).insert(blade);
        }
        None => {
            commands.entity(body).remove::<crate::making::Blade>();
        }
    }
    hero.wear(&arms.form, assets);
    match size {
        Some(mut size) => size.0 = arms.form.size,
        None => {
            commands.entity(body).insert(hero::Size(arms.form.size));
        }
    }
    let level = experience.level;
    let skills = fight::hero_skills(base, level, &holds);
    let list = effects.list();
    *combat = Combat(combat_with(
        base,
        level,
        holds.clone(),
        &skills,
        &list,
        rules,
    ));
    regenerates.value = regeneration_with(base, level, &skills, &list, rules);
    commands.entity(body).insert(Holding(holds));
    hero.pace = combat.pace_factor();
    hero.speeds = arms.form.speeds.map(|s| f32::from(s) * hero.pace * speed.0);
}

/// The dice of the weapons' sparks (`rand()`).
#[derive(Resource)]
struct Sparks(crate::spawning::Rng);

impl Default for Sparks {
    fn default() -> Self {
        Self(crate::seed::dice(0x856))
    }
}

/// Bursts each weapon in the hero's hands into sparks of its shape as its effect
/// [`arts::RAILGUN`] comes, before [`bear`] takes the weapons out of its hands: the adder's case
/// 17 (`0x582D50`) makes an object 856 of the variant 2 at each, whose start takes every
/// `⌊V/500⌋`th vertex of the item's model as last drawn (`0x767FB0`,
/// `opensacred_rules::particles::{vertex_picks, weapon_sparks}`), drawn by `puffs`.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn shatter(
    mut commands: Commands,
    arsenal: Option<Res<Arsenal>>,
    bodies: Query<(Entity, &Effects, &GlobalTransform), (With<Body>, Changed<Effects>)>,
    weapons: Query<(Entity, &crate::models::OrderedWith), With<crate::npcs::Wielded>>,
    (children, shapes): (Query<&Children>, Query<(&Mesh3d, &GlobalTransform)>),
    (meshes, mut dice): (Res<Assets<Mesh>>, ResMut<Sparks>),
) {
    let Some(arsenal) = arsenal else {
        return;
    };
    if arsenal.bolts.is_none() || arsenal.borne == Some(arts::RAILGUN) {
        return;
    }
    for (body, effects, at) in &bodies {
        if !effects.holds(arts::RAILGUN) || effects.holds(arts::BFG) {
            continue;
        }
        let origin = at.translation();
        for (weapon, _) in weapons.iter().filter(|(_, with)| with.0 == body) {
            let parts: Vec<(Vec<[f32; 3]>, &GlobalTransform)> = std::iter::once(weapon)
                .chain(children.iter_descendants(weapon))
                .filter_map(|entity| shapes.get(entity).ok())
                .filter_map(|(mesh, place)| {
                    let positions = meshes
                        .get(&mesh.0)?
                        .attribute(Mesh::ATTRIBUTE_POSITION)?
                        .as_float3()?;
                    Some((positions.to_vec(), place))
                })
                .collect();
            let counts: Vec<usize> = parts.iter().map(|(positions, _)| positions.len()).collect();
            let points: Vec<[f32; 3]> = opensacred_rules::particles::vertex_picks(&counts)
                .into_iter()
                .filter_map(|(mesh, vertex)| {
                    let (positions, place) = parts.get(mesh)?;
                    let point = Vec3::from_array(*positions.get(vertex)?);
                    Some(crate::puffs::local(place.transform_point(point), origin))
                })
                .collect();
            if points.is_empty() {
                continue;
            }
            debug!(
                "arsenal: the weapon {weapon} bursts into {} sparks",
                points.len()
            );
            crate::puffs::spawn(
                &mut commands,
                opensacred_rules::particles::weapon_sparks(
                    &points,
                    opensacred_rules::particles::WEAPON_SPARKS,
                    &mut dice.0,
                ),
                origin,
                None,
            );
        }
    }
}

/// Gives the hero the arms of its effect [`arts::BFG`] or [`arts::RAILGUN`] as the effect comes,
/// and its own back as it goes.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn bear(
    mut commands: Commands,
    arsenal: Option<ResMut<Arsenal>>,
    mut hero: ResMut<Hero>,
    assets: Res<AssetServer>,
    (experience, rules, speed): (Res<Experience>, Res<Rules>, Res<GameSpeed>),
    mut bodies: Query<
        (
            Entity,
            &Base,
            &Effects,
            &mut Combat,
            &mut Regenerates,
            Option<&mut hero::Size>,
            Option<&Worn>,
            Option<&Holding>,
        ),
        (With<Body>, Changed<Effects>),
    >,
) {
    let Some(mut arsenal) = arsenal else {
        return;
    };
    if arsenal.bfg.is_none() && arsenal.bolts.is_none() {
        return;
    }
    for wearer in &mut bodies {
        let borne = [(arts::BFG, &arsenal.bfg), (arts::RAILGUN, &arsenal.bolts)]
            .into_iter()
            .find(|(effect, arms)| arms.is_some() && wearer.2.holds(*effect));
        let wanted = borne.map(|(effect, _)| effect);
        if wanted == arsenal.borne {
            continue;
        }
        let arms = borne
            .and_then(|(_, arms)| arms.clone())
            .unwrap_or_else(|| arsenal.own.clone());
        arsenal.borne = wanted;
        info!("arsenal: the hero bears the arms of the effect {wanted:?}");
        wear(
            arms,
            &mut commands,
            &mut hero,
            &assets,
            (&experience, &rules, &speed),
            wearer,
        );
    }
}

/// Gives the hero its arms without the item it hurled as it hurls it, and its own back as it
/// picks the item up again (#755).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn throw(
    mut commands: Commands,
    mut throws: MessageReader<Throws>,
    arsenal: Option<ResMut<Arsenal>>,
    mut hero: ResMut<Hero>,
    assets: Res<AssetServer>,
    (experience, rules, speed): (Res<Experience>, Res<Rules>, Res<GameSpeed>),
    mut bodies: Query<
        (
            Entity,
            &Base,
            &Effects,
            &mut Combat,
            &mut Regenerates,
            Option<&mut hero::Size>,
            Option<&Worn>,
            Option<&Holding>,
        ),
        With<Body>,
    >,
) {
    let Some(mut arsenal) = arsenal else {
        throws.clear();
        return;
    };
    for &Throws(thrown) in throws.read() {
        let Some(hurled) = arsenal.hurled.clone() else {
            continue;
        };
        if thrown == arsenal.thrown {
            continue;
        }
        arsenal.thrown = thrown;
        let arms = if thrown {
            hurled.arms
        } else {
            arsenal.own.clone()
        };
        info!(
            "arsenal: the hero {} the item {}",
            if thrown { "hurls" } else { "takes back" },
            hurled.item.item
        );
        for wearer in &mut bodies {
            wear(
                arms.clone(),
                &mut commands,
                &mut hero,
                &assets,
                (&experience, &rules, &speed),
                wearer,
            );
        }
    }
}

/// Gives the hero the arms of the items now in its hands (#1348), its own from then on; what its
/// effect [`arts::BFG`] puts in its hand, or its hand that hurled its item, holds on until it
/// ends, the new arms coming then. A hero that has had no effect yet has none.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn rearm(
    mut commands: Commands,
    mut rearms: MessageReader<Rearm>,
    mut arsenal: Option<ResMut<Arsenal>>,
    mut hero: ResMut<Hero>,
    assets: Res<AssetServer>,
    (experience, rules, speed): (Res<Experience>, Res<Rules>, Res<GameSpeed>),
    mut bodies: Query<
        (
            Entity,
            &Base,
            Option<&Effects>,
            &mut Combat,
            &mut Regenerates,
            Option<&mut hero::Size>,
            Option<&Worn>,
            Option<&Holding>,
        ),
        With<Body>,
    >,
) {
    let none = Effects::default();
    for Rearm(arms) in rearms.read() {
        debug!(
            "arsenal: the hero's arms made anew, {} items shown",
            arms.held.len()
        );
        if let Some(arsenal) = arsenal.as_deref_mut() {
            arsenal.own = arms.clone();
            if arsenal.borne.is_some() || arsenal.thrown {
                continue;
            }
        }
        for (body, base, effects, combat, regenerates, size, worn, holding) in &mut bodies {
            wear(
                arms.clone(),
                &mut commands,
                &mut hero,
                &assets,
                (&experience, &rules, &speed),
                (
                    body,
                    base,
                    effects.unwrap_or(&none),
                    combat,
                    regenerates,
                    size,
                    worn,
                    holding,
                ),
            );
        }
    }
}

/// Takes the arsenal back with the game.
fn leave(mut commands: Commands) {
    commands.remove_resource::<Arsenal>();
    commands.remove_resource::<Maker>();
}

pub struct ArsenalPlugin;

impl Plugin for ArsenalPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Throws>()
            .add_message::<Rearm>()
            .init_resource::<Sparks>()
            .add_systems(
                Update,
                (shatter, bear, throw, rearm)
                    .chain()
                    .in_set(crate::frame::Step::HeroSpawn),
            )
            .add_systems(OnExit(AppState::InGame), leave);
    }
}
