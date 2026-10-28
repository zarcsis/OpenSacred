//! The arms that the heroes' arts change. The Seraphim's 1018 (#754; docs/re/arts.md, "The
//! Seraphim's"): while her effect [`arts::BFG`] holds, her hands hold the item [`arts::BFG_ITEM`]
//! alone, with its values, its model and the clips of its class, her own items put away (the
//! adder `0x582D50`); as it ends the item goes and her own come back (the taker `0x586D80`; seen
//! 2026-10-24: her sword back in her hand and her values as before, 30 s after the art at the
//! level 1). The Gladiator's 1013 (#755; docs/re/arts.md, "The Gladiator's"): the item it hurls
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

/// The hero's own arms, those of the Seraphim's effect [`arts::BFG`], the Gladiator's without the
/// item he hurls, and which it bears.
#[derive(Resource)]
pub struct Arsenal {
    own: Arms,
    bfg: Option<Arms>,
    hurled: Option<Hurled>,
    bearing: bool,
    thrown: bool,
}

impl Arsenal {
    pub fn new(own: Arms) -> Self {
        Self {
            own,
            bfg: None,
            hurled: None,
            bearing: false,
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
    (body, base, effects, mut combat, mut regenerates, size, worn): Wearer,
) {
    let mut holds = arms.holds;
    holds
        .items
        .extend(worn.into_iter().flat_map(|worn| worn.0.iter().cloned()));
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
    let skills = fight::skills(base, level, true);
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

/// Gives the hero the arms of its effect [`arts::BFG`] as the effect comes, and its own back as
/// it goes.
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
        ),
        (With<Body>, Changed<Effects>),
    >,
) {
    let Some(mut arsenal) = arsenal else {
        return;
    };
    for wearer in &mut bodies {
        let Some(bfg) = arsenal.bfg.clone() else {
            return;
        };
        let bearing = wearer.2.holds(arts::BFG);
        if bearing == arsenal.bearing {
            continue;
        }
        arsenal.bearing = bearing;
        let arms = if bearing { bfg } else { arsenal.own.clone() };
        info!(
            "arsenal: the hero {} the item {}",
            if bearing { "takes" } else { "puts away" },
            arts::BFG_ITEM
        );
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
            if arsenal.bearing || arsenal.thrown {
                continue;
            }
        }
        for (body, base, effects, combat, regenerates, size, worn) in &mut bodies {
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
            .add_systems(
                Update,
                (bear, throw, rearm)
                    .chain()
                    .in_set(crate::frame::Step::HeroSpawn),
            )
            .add_systems(OnExit(AppState::InGame), leave);
    }
}
