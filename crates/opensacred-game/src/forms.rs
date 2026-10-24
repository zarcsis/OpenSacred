//! The Vampiress's forms (#758; docs/re/arts.md, "The Vampiress's"): by day her type 6 and
//! `VLADY_D.GRN`, by night her type 7 and `VLADY_N.GRN`, its record's values and its claws' clips
//! (the class 0, whatever she holds). Her changeling 1022 gives her the night's form for the
//! effect 22's time, and 1023, or the effect's end, gives her the day's back (`0x50E6C0`, its
//! event of her change; seen: her values the other record's, her health's most made anew and its
//! current kept within it).

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Result, anyhow};
use bevy::prelude::*;
use opensacred_rules::arts as rules;
use opensacred_rules::fight::{Effect, combat_with, hero_health, regeneration_with};

use crate::arts::HeroArts;
use crate::daynight::HeroLight;
use crate::effects::Effects;
use crate::fight::{self, Base, Combat, Experience, Health, Holding, Regenerates, Rules};
use crate::hero::{self, Body, GameSpeed, Hero};
use crate::sound::Footsteps;

/// The night's model, and the presets of its idle, walk and run where its slots cannot be read.
const NIGHT_MODEL: &str = "VLADY_N.GRN";
const NIGHT_CLIPS: [&str; 3] = ["VMPN_IDLE_BH", "VMPN_WALK_BH", "VMPN_RUN_BH"];
/// What the Vampiress fights with by night: her claws, the class 0 (docs/re/motions.md).
const CLAWS: u8 = 0;

/// The clips of a hero's model for what it holds: its idle, walk and run, its fight stance, its
/// attacks with the fractions at which they hit, its arts' actions and its dying clips.
#[derive(Clone, Debug, Default)]
pub struct Clips {
    pub moving: [String; 3],
    pub stance: Option<String>,
    pub attacks: Vec<(String, [f32; 2])>,
    pub arts: Vec<hero::ArtClipName>,
    pub dying: Vec<String>,
}

/// A form of the hero: its type, its model and clips, its record's values for its fight, its
/// record's walk and run speeds (`Creature.pak +0x26`, `+0x28`), its size, and whether its hands
/// show what it holds (by night the Vampiress fights with her claws, her weapons not drawn: seen).
#[derive(Clone, Debug)]
pub struct Form {
    pub creature: u32,
    pub model: String,
    pub clips: Clips,
    pub base: Base,
    pub speeds: [u16; 2],
    pub size: u32,
    pub armed: bool,
}

impl Form {
    /// The Vampiress's night's form: `VLADY_N.GRN` and its claws' clips, her type 7's record's
    /// base attributes and speeds, and her day's values (`day`) for the rest: her stats keep their
    /// type, 6, by which the recomputation takes the record's bonuses, the damping and the skills
    /// (`0x5796A0` at `0x5797FD`; seen: `+0x08` 6 by night, #792).
    pub fn night(
        assets: &Path,
        hits: &HashMap<String, [f32; 2]>,
        points: &HashMap<String, Vec<f32>>,
        day: &Base,
    ) -> Result<Self> {
        let model = crate::npcs::model_files(assets)?
            .remove(NIGHT_MODEL)
            .ok_or_else(|| anyhow!("no model {NIGHT_MODEL} in {}", assets.display()))?;
        let record = crate::game::record(assets, rules::VAMPIRESS_BY_NIGHT)?;
        Ok(Self {
            creature: rules::VAMPIRESS_BY_NIGHT,
            clips: crate::game::clips_of(
                assets,
                &model,
                NIGHT_CLIPS,
                (rules::VAMPIRESS_BY_NIGHT, CLAWS),
                hits,
                points,
            ),
            model,
            base: Base(opensacred_rules::fight::Base {
                attributes: record.base,
                ..day.0
            }),
            speeds: record.speed,
            size: opensacred_rules::minds::size(rules::VAMPIRESS_BY_NIGHT, false),
            armed: false,
        })
    }
}

/// The hero's two forms, where its class has them: whether it wears its night's, and the footsteps
/// of the form it does not wear.
#[derive(Resource)]
pub struct Forms {
    day: Form,
    night: Form,
    by_night: bool,
    steps: Footsteps,
}

impl Forms {
    pub fn new(day: Form, night: Form, steps: Footsteps) -> Self {
        Self {
            day,
            night,
            by_night: false,
            steps,
        }
    }
}

/// A change of the hero's form: into its night's for `seconds`, or back to its day's.
#[derive(Event, Clone, Copy, Debug, PartialEq)]
pub struct Change {
    pub night: bool,
    pub seconds: f32,
}

/// The hero's change of form (`0x50E6C0`'s event of her change): its model and clips, its type
/// for its arts and its light, its record's values, its health's most made anew with its current
/// kept within it, its speeds by its pace; into the night's form with the effect 22 for its time,
/// back to the day's with every effect 22 taken off.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy observer takes its resources and queries as arguments, its queries with their \
              filters"
)]
fn change(
    change: On<Change>,
    mut commands: Commands,
    forms: Option<ResMut<Forms>>,
    mut hero: ResMut<Hero>,
    assets: Res<AssetServer>,
    (mut arts, mut light, mut footsteps): (
        Option<ResMut<HeroArts>>,
        ResMut<HeroLight>,
        Option<ResMut<Footsteps>>,
    ),
    (experience, rules, speed): (Res<Experience>, Res<Rules>, Res<GameSpeed>),
    mut bodies: Query<
        (
            Entity,
            &mut Base,
            &mut Health,
            &mut Combat,
            &mut Regenerates,
            &mut hero::Size,
            Option<&Holding>,
            Option<&mut Effects>,
        ),
        With<Body>,
    >,
) {
    let Some(mut forms) = forms else {
        return;
    };
    if forms.by_night == change.night {
        return;
    }
    forms.by_night = change.night;
    let form = if change.night {
        forms.night.clone()
    } else {
        forms.day.clone()
    };
    info!(
        "forms: the hero takes its {} form, type {}",
        if change.night { "night's" } else { "day's" },
        form.creature
    );
    hero.wear(&form, &assets);
    if let Some(arts) = arts.as_mut() {
        arts.kind = form.creature;
    }
    // The Vampiress's light by night is doubled as the Wood Elf's (class 7, `0x54A7B0`).
    light.doubled = change.night;
    if let Some(footsteps) = footsteps.as_mut() {
        std::mem::swap(&mut **footsteps, &mut forms.steps);
    }
    for (body, mut base, mut health, mut combat, mut regenerates, mut size, holding, effects) in
        &mut bodies
    {
        *base = form.base;
        size.0 = form.size;
        let effect = Effect::new(rules::NIGHT_FORM, 0.0);
        let list = match effects {
            Some(mut effects) => {
                if change.night {
                    effects.add(effect, change.seconds);
                } else {
                    effects.remove(rules::NIGHT_FORM);
                }
                effects.list()
            }
            None => {
                let mut effects = Effects::default();
                if change.night {
                    effects.add(effect, change.seconds);
                }
                let list = effects.list();
                commands.entity(body).insert(effects);
                list
            }
        };
        let level = experience.level;
        let skills = fight::skills(&base, level, true);
        let holds = holding.map(|holding| holding.0.clone()).unwrap_or_default();
        *combat = Combat(combat_with(&base, level, holds, &skills, &list, &rules));
        regenerates.value = regeneration_with(&base, level, &skills, &list, &rules);
        let most = hero_health(base.attributes, level, &skills, &rules);
        *health = Health {
            current: health.current.min(most),
            most,
            level,
        };
        hero.pace = combat.pace_factor();
        hero.speeds = form.speeds.map(|s| f32::from(s) * hero.pace * speed.0);
    }
}

/// The night's form ends with its effect (`0x50E6C0`: 1023 takes every effect 22 off her and
/// gives her back her day's form; **hypothesis**: the effect's end does the same).
fn wear_off(
    mut commands: Commands,
    forms: Option<Res<Forms>>,
    heroes: Query<&Effects, (With<Body>, Changed<Effects>)>,
) {
    if !forms.is_some_and(|forms| forms.by_night) {
        return;
    }
    if heroes
        .iter()
        .any(|effects| !effects.holds(rules::NIGHT_FORM))
    {
        commands.trigger(Change {
            night: false,
            seconds: 0.0,
        });
    }
}

pub struct FormsPlugin;

impl Plugin for FormsPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(change)
            .add_systems(Update, wear_off.in_set(crate::frame::Step::HeroSpawn));
    }
}
