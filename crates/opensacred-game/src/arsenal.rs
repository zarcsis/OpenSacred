//! The Seraphim's 1018 (#754; docs/re/arts.md, "The Seraphim's"): while her effect
//! [`arts::BFG`] holds, her hands hold the item [`arts::BFG_ITEM`] alone, with its values, its
//! model and the clips of its class, her own items put away (the adder `0x582D50`); as it ends the
//! item goes and her own come back (the taker `0x586D80`; seen 2026-10-24: her sword back in her
//! hand and her values as before, 30 s after the art at the level 1).

use bevy::prelude::*;
use opensacred_rules::arts;
use opensacred_rules::fight::{combat_with, regeneration_with};

use crate::effects::Effects;
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

/// The Seraphim's own arms and those of her effect [`arts::BFG`], and which she bears.
#[derive(Resource)]
pub struct Arsenal {
    own: Arms,
    bfg: Arms,
    bearing: bool,
}

impl Arsenal {
    pub fn new(own: Arms, bfg: Arms) -> Self {
        Self {
            own,
            bfg,
            bearing: false,
        }
    }
}

/// Gives the hero the arms of its effect [`arts::BFG`] as the effect comes, and its own back as
/// it goes: its items' models and clips, the values of what it holds, its size, and its values
/// made anew with them, its pace and speeds with them.
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
        ),
        (With<Body>, Changed<Effects>),
    >,
) {
    let Some(mut arsenal) = arsenal else {
        return;
    };
    for (body, base, effects, mut combat, mut regenerates, size) in &mut bodies {
        let bearing = effects.holds(arts::BFG);
        if bearing == arsenal.bearing {
            continue;
        }
        arsenal.bearing = bearing;
        let arms = if bearing {
            arsenal.bfg.clone()
        } else {
            arsenal.own.clone()
        };
        info!(
            "arsenal: the hero {} the item {}",
            if bearing { "takes" } else { "puts away" },
            arts::BFG_ITEM
        );
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
        hero.wear(&arms.form, &assets);
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
            arms.holds.clone(),
            &skills,
            &list,
            &rules,
        ));
        regenerates.value = regeneration_with(base, level, &skills, &list, &rules);
        commands.entity(body).insert(Holding(arms.holds));
        hero.pace = combat.pace_factor();
        hero.speeds = arms.form.speeds.map(|s| f32::from(s) * hero.pace * speed.0);
    }
}

/// Takes the arsenal back with the game.
fn leave(mut commands: Commands) {
    commands.remove_resource::<Arsenal>();
}

pub struct ArsenalPlugin;

impl Plugin for ArsenalPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, bear.in_set(crate::frame::Step::HeroSpawn))
            .add_systems(OnExit(AppState::InGame), leave);
    }
}
