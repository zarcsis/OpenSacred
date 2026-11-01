//! The Daemon's form of 1094 as its effect 50 holds it (#918; docs/re/arts.md, "The effects",
//! 50): in it he wades where the ground is low (`hero::run`), and his steps sound as wings
//! (`sound`); a building's or a settlement's cell takes it off him; a number counts its last
//! seconds down over him; and as it ends where he may not stand he is put on the ground he may
//! about him, or, with none, drowns until he stands where he may or dies.

use bevy::prelude::*;
use opensacred_rules::arts::{self as rules, DAEMON_FORM, DROWNS, FORM, FORMED};
use opensacred_rules::fight::Effect;
use opensacred_rules::spawning::Rng;

use crate::effects::{Effects, Wears};
use crate::fight::{Health, Hit};
use crate::frame::{Step, WorldTick};
use crate::hero::Body;
use crate::lines::Says;
use crate::npcs::Dies;
use crate::paths;
use crate::terrain::Heights;

/// The dice of where his form's end puts him.
#[derive(Resource)]
struct Dice(Rng);

/// A building's or a settlement's cell takes his form off the Daemon: on a cell of the ground's
/// flag bit 0 (`0x635240`) every effect keyed by 1094 and [`DAEMON_FORM`] ends, its seconds set to
/// 0 (`0x5888F0`), so that [`ends`] puts him where he may stand, but while he makes an art
/// (`0x594C42` in `0x5941A0`, his action not 15); the inside layers' cells have no such flag.
fn towns(
    heights: Res<Heights>,
    mut heroes: Query<(&Transform, &Body, &mut Effects), Without<crate::making::Playing>>,
) {
    for (transform, body, mut effects) in &mut heroes {
        if body.level().is_some() || !effects.holds_any(&[FORM]) {
            continue;
        }
        let at = transform.translation;
        let cell = paths::cell_of(at.x, at.z);
        if heights
            .flags(cell)
            .is_some_and(|flags| flags & crate::buildings::BUILDING_CELL != 0)
        {
            debug!("daemon: the cell {cell:?} takes his form off");
            effects.end(&[u32::from(FORM), DAEMON_FORM]);
        }
    }
}

/// The colour of an ARGB word.
fn argb(colour: u32) -> Color {
    let [alpha, red, green, blue] = colour.to_be_bytes();
    Color::srgba_u8(red, green, blue, alpha)
}

/// The form's end, at each tick before the effects wear (`0x5635BB`, its taker `0x5881A0`): a
/// number over him counts its last seconds down ([`rules::countdown`]); as it goes where he may
/// walk the drowning goes with it; where he may not, he is put on the first ring about him that
/// holds a cell he may ([`rules::landing`]); with none, the form holds 1 s more and gives him,
/// unless one holds, the drowning's effect 55 ([`rules::drowning`]), which holds while the form
/// does (**hypothesis**: what ends it is not read). At his death both go.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn ends(
    mut commands: Commands,
    tick: Res<WorldTick>,
    heights: Res<Heights>,
    mut dice: ResMut<Dice>,
    mut heroes: Query<(
        Entity,
        &mut Transform,
        &mut Body,
        &mut Effects,
        &Health,
        Has<Dies>,
    )>,
) {
    let Some(seconds) = tick.seconds else {
        return;
    };
    for (hero, mut transform, mut body, mut effects, health, dead) in &mut heroes {
        let Some(left) = effects.left(FORMED) else {
            continue;
        };
        if dead {
            effects.remove(DROWNS);
            effects.remove(FORMED);
            continue;
        }
        let at = transform.translation;
        let cell = paths::cell_of(at.x, at.z);
        let walkable = heights.walkable_on(cell, body.level());
        let after = left - seconds;
        if let Some((number, colour)) = rules::countdown(left, after, walkable) {
            commands
                .entity(hero)
                .insert(Says::own(number.to_string(), argb(colour)));
        }
        // The drowning holds while the form does, 5 s more each time while he lives.
        if effects
            .left(DROWNS)
            .is_some_and(|drowns| drowns - seconds <= 0.0)
        {
            effects.prolong(DROWNS, rules::DROWNING_SECONDS);
        }
        if after > 0.0 {
            continue;
        }
        if walkable {
            effects.remove(DROWNS);
            continue;
        }
        if let Some(dry) = rules::landing(cell, |next| heights.walkable_on(next, None), &mut dice.0)
        {
            debug!("daemon: his form ends off the ground, he is put on {dry:?}");
            let centre = paths::centre(dry);
            body.teleport(&mut transform, Vec3::new(centre.x, at.y, centre.y));
            effects.remove(DROWNS);
            continue;
        }
        effects.prolong(FORMED, 1.0);
        if !effects.holds(DROWNS) {
            let whole = rules::drowning(health.current);
            debug!("daemon: his form ends off the ground, none about him, he drowns by {whole}");
            effects.add(
                Effect {
                    id: DROWNS,
                    whole: i32::try_from(whole).unwrap_or(i32::MAX),
                    value: 0.0,
                    art: DAEMON_FORM,
                    level: 0,
                },
                rules::DROWNING_SECONDS,
            );
        }
    }
}

/// The drowning's strikes: every 0.5 s while its effect 55 holds, its whole value from himself,
/// with no roll of the hit (`0x5881A0`, the event 5 of the hit's kind `0x11`; **hypothesis**:
/// physical damage).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn drown(
    tick: Res<WorldTick>,
    mut hits: MessageWriter<Hit>,
    mut since: Local<f32>,
    heroes: Query<(Entity, &Effects), (With<Body>, Without<Dies>)>,
) {
    let Some(seconds) = tick.seconds else {
        return;
    };
    for (hero, effects) in &heroes {
        let Some(whole) = effects
            .timed()
            .iter()
            .find(|timed| timed.effect.id == DROWNS)
            .map(|timed| timed.effect.whole)
        else {
            *since = 0.0;
            continue;
        };
        *since += seconds;
        while *since >= rules::DROWNING_EVERY {
            *since -= rules::DROWNING_EVERY;
            debug!("daemon: he drowns, {whole}");
            hits.write(Hit {
                attacker: hero,
                target: hero,
                share: 1.0,
                ranged: false,
                art: None,
                fixed: Some([
                    f32::from(u16::try_from(whole).unwrap_or(u16::MAX)),
                    0.0,
                    0.0,
                    0.0,
                ]),
                sure: true,
                poison: false,
            });
        }
    }
}

/// The Daemon's form.
pub struct DaemonPlugin;

impl Plugin for DaemonPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Dice(crate::seed::dice(11)))
            .add_systems(
                Update,
                (towns, ends, drown)
                    .chain()
                    .run_if(crate::frame::ticked)
                    .before(Wears)
                    .in_set(crate::frame::Unpaused)
                    .in_set(Step::Crowd),
            );
    }
}
