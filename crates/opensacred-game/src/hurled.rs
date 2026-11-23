//! The Gladiator's hurled weapon (1013, #755; docs/re/arts.md, "The Gladiator's"). As below 0.6
//! of the art's clip is left (`making`), the weapon in his left hand, else his right, leaves it
//! (`0x556A30`, his arms then without it, `arsenal`) and flies on an arc from him to 200 beyond
//! his target along the line to it (`0x501D39`), 80 high, for the ground distance to the target
//! over 1,000 seconds (`0x44DFE0`'s arc branch). It strikes the first creature that it touches
//! by the sum of their sizes, alive and not the hero (`0x600340`), once, with its own damages and
//! its bonus slots' by his attributes (#3372), those against the struck one's class among them
//! (#3383), times `1 + c(s₂)/100 + a0/BalanceGeschick`, each rolled between its least and most,
//! in the place of his attack record's, so without the survival's raise (#3382), and the art's
//! factor (`0x56B130` at `0x56BCE2`); its hit takes the arts' roll while his action is still the
//! art's, its clip playing and no walk or attack ordered since, and lands without one after
//! (`0x56BC81`, #3381); then it falls there, or at its end, an item lying on the ground, which
//! the hero picks up again into his hand (`pickup`).

use bevy::prelude::*;
use opensacred_rules::arts;

use crate::arsenal::{Arsenal, Throws};
use crate::arts::HeroArts;
use crate::drops::Lying;
use crate::fight::{self, Combat, Health};
use crate::frame::{Playing, Step, Unpaused};
use crate::game::AppState;
use crate::hero::Body;
use crate::npcs::{Dies, Retexture};
use crate::spawning::Rng;
use crate::terrain::{Heights, Level};
use crate::{iso, models};

/// The hero's throw of its weapon, as the art's clip reaches it (`making`): where its target
/// stood, the art's level in play and its factor, and the seconds its clip still plays, through
/// which the hero's action stays the art's (`0x4FF0B0`; docs/re/arts.md, "The Gladiator's").
#[derive(Component, Clone, Copy, Debug)]
pub struct Hurls {
    pub at: Vec3,
    pub level: u32,
    pub factor: f32,
    pub clip_left: f32,
}

/// A weapon in flight: its arc's ends on the ground, its height, its time and the time flown, the
/// level it flies on, its thrower, the art's level and factor, the seconds the art's clip played
/// on after the throw, and its size.
#[derive(Component)]
struct Flying {
    from: Vec2,
    to: Vec2,
    height: f32,
    time: f32,
    flown: f32,
    level: Level,
    thrower: Entity,
    art: (u32, f32),
    clip_left: f32,
    size: f32,
}

/// Whether the hurled weapon's thrower is still in the art's action at its hit, by which the hit
/// takes the arts' roll (`0x56BC81`, #3381): its clip still playing, `flown` seconds after the
/// throw of `clip_left` seconds left, the thrower `alive` and `left` nothing for another action:
/// a walk (whose click sets the action 1 at the next tick, `0x4EE289`), an attack (whose order
/// sets it 0, `0x4E06DD`), a knock (the event 3, `0x54E6D0`). Another art ordered while the clip
/// plays leaves the action as it is.
fn in_the_art(flown: f32, clip_left: f32, alive: bool, left: bool) -> bool {
    flown < clip_left && alive && !left
}

/// The dice of the hurled weapons' damages.
#[derive(Resource)]
struct Dice(Rng);

/// The creature that a hurled item strikes, against which its slots of a class of opponents count
/// (`fight::HurledAt`, #3383): its side, whether it holds a ranged weapon or is of the type's
/// mark 8, and whether it rides a horse or is one ridden; the thrower of the hero's type
/// `thrower`.
fn struck_at(
    thrower: u32,
    mind: Option<&crate::ai::Mind>,
    arms: Option<&crate::npcs::Arms>,
    ridden: bool,
) -> opensacred_rules::fight::HurledAt {
    opensacred_rules::fight::HurledAt {
        thrower,
        side: mind.map_or(0, |mind| mind.side),
        ranged: arms.is_some_and(crate::npcs::Arms::ranged)
            || mind.is_some_and(|mind| mind.mark() & 0xFFF == MAGICIANS),
        mounted: ridden || mind.is_some_and(|mind| mind.rides.is_some()),
    }
}

/// The mark of the type's magicians (`+0x25F`'s low 12 bits), whom the slots against the ranged
/// count against with those that hold a ranged weapon (`0x80`, docs/re/arts.md, "The art lines
/// by id").
const MAGICIANS: u16 = 8;

/// World units of a height of `height` units of the models' height, as a drop's (`drops`).
fn up(height: f32) -> f32 {
    iso::height(height * iso::PIXELS_PER_MODEL_HEIGHT)
}

/// The hero's weapon leaves its hand as its throw comes: its arms without it, and the weapon in
/// flight from its place toward the end of the arc.
fn throw(
    mut commands: Commands,
    arsenal: Option<Res<Arsenal>>,
    mut throws: MessageWriter<Throws>,
    assets: Res<AssetServer>,
    heroes: Query<(Entity, &Transform, &Body, &Hurls)>,
) {
    for (hero, transform, body, hurls) in &heroes {
        commands.entity(hero).remove::<Hurls>();
        let Some(hurled) = arsenal.as_deref().and_then(Arsenal::hurls) else {
            continue;
        };
        let from = transform.translation.xz();
        let (to, d) = arts::throw_end(from.into(), hurls.at.xz().into(), true);
        info!(
            "hurled: the hero hurls {} toward {:.0},{:.0}",
            hurled.item.item, to[0], to[1]
        );
        let mut entity = commands.spawn((
            Transform::from_translation(transform.translation)
                .with_scale(Vec3::splat(iso::MODEL_SCALE)),
            models::scene(&assets, &hurled.held.model),
            models::Ordered::default(),
            Flying {
                from,
                to: to.into(),
                height: arts::throw_height(true),
                time: arts::throw_time(d),
                flown: 0.0,
                level: body.level(),
                thrower: hero,
                art: (hurls.level, hurls.factor),
                clip_left: hurls.clip_left,
                size: crate::arrows::size(0),
            },
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(texture) = &hurled.held.texture {
            entity.insert(Retexture::load(&assets, texture));
        }
        throws.write(Throws(true));
    }
}

/// Flies each hurled weapon on its arc a frame on: it strikes the first creature it touches, and
/// falls there or at its arc's end, an item lying on the ground.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    heights: Res<Heights>,
    rules: Res<fight::Rules>,
    arts: Option<Res<HeroArts>>,
    arsenal: Option<Res<Arsenal>>,
    mut dice: ResMut<Dice>,
    mut hits: MessageWriter<fight::Hit>,
    mut flying: Query<(Entity, &mut Transform, &mut Flying)>,
    throwers: Query<(
        &Combat,
        Option<&crate::fight::Base>,
        Option<&Health>,
        Option<&Body>,
        (
            Has<crate::attack::Attack>,
            Has<fight::Pushed>,
            Has<fight::Downed>,
        ),
    )>,
    creatures: Query<
        (
            Entity,
            &GlobalTransform,
            &Health,
            Option<&crate::arrows::Size>,
            (
                Option<&crate::ai::Mind>,
                Option<&crate::npcs::Arms>,
                Has<crate::riding::Ridden>,
            ),
        ),
        (Without<Dies>, Without<Body>, Without<Flying>),
    >,
) {
    for (entity, mut transform, mut flight) in &mut flying {
        flight.flown += time.delta_secs();
        let share = flight.flown / flight.time.max(f32::EPSILON);
        let (at, rise) =
            arts::throw_point(flight.from.into(), flight.to.into(), share, flight.height);
        let at = Vec2::from(at);
        let ground = heights
            .standing(at.x, at.y, flight.level)
            .unwrap_or(transform.translation.y);
        transform.translation = Vec3::new(at.x, ground + up(rise), at.y);
        // The first creature it touches, alive, by the sum of their sizes (`0x600340`).
        let thrower_type = arts.as_deref().map_or(0, |arts| arts.kind);
        let struck = creatures
            .iter()
            .filter(|(_, _, health, ..)| health.current > 0)
            .find(|(_, place, _, size, _)| {
                let size = size.map_or(crate::arrows::size(0), |size| size.0);
                (place.translation().xz() - at).length() <= flight.size + size
            })
            .map(|(target, _, _, _, (mind, arms, ridden))| {
                (target, struck_at(thrower_type, mind, arms, ridden))
            });
        if let (Some((target, struck_at)), Some(item)) = (
            struck,
            arsenal
                .as_deref()
                .and_then(|arsenal| arsenal.hurled_item())
                .and_then(|item| item.values),
        ) {
            // Its own damages and its slots' by the thrower's attributes (#3372), those against
            // the struck one's class among them (#3383), times
            // `1 + c(s₂)/100 + (a0 + e0)/BalanceGeschick`, `e0` the points spent on `a0`, which
            // `a0` holds already (`0x56ACD0`, the stats' byte `+0x1C`, #3135), each rolled at the
            // hundredth between its least and most.
            let thrower = throwers.get(flight.thrower).ok();
            let attributes = thrower.map_or([0.0; 6], |(combat, ..)| combat.attributes);
            let strength = thrower.map_or(0.0, |(combat, base, ..)| {
                combat.attributes[0] + base.map_or(0.0, |base| f32::from(base.extras[0]))
            });
            let in_art =
                thrower.is_some_and(|(_, _, health, body, (attacking, pushed, downed))| {
                    let alive = health.is_none_or(|health| health.current > 0);
                    let walking = body.is_some_and(|body| body.moving().is_some());
                    let left = walking || attacking || pushed || downed;
                    in_the_art(flight.flown, flight.clip_left, alive, left)
                });
            let skills = arts
                .as_deref()
                .map(|arts| arts.skills.clone())
                .unwrap_or_default();
            let times = rules.glance_factor(&skills, strength);
            let fixed = arts::thrown_damage(
                &item.hurled_damage(attributes, Some(struck_at)),
                times,
                &mut dice.0,
            );
            info!(
                "hurled: the hurled weapon strikes {target}, {:?}, {}",
                fixed.map(|d| d.round()),
                if in_art { "the arts' roll" } else { "no roll" }
            );
            // Its slot against the undead marks an undead that it strikes (`0x56ACD0` at
            // `0x56AE38`, #3385).
            if struck_at.side == opensacred_rules::undead::CLASS
                && opensacred_rules::fight::marks_undead(std::slice::from_ref(&item), thrower_type)
            {
                commands.entity(target).insert(crate::undead::Marked);
            }
            hits.write(fight::Hit {
                attacker: flight.thrower,
                target,
                share: flight.art.1,
                ranged: false,
                art: Some(flight.art.0),
                fixed: Some(fixed),
                // The arts' roll by the thrower's `A` of what he holds now, the weapon out of his
                // hand, while his action is the art's; none after (the kind 3 never takes the
                // plain one).
                sure: !in_art,
                poison: false,
                ward: false,
                hurled: true,
            });
        }
        if struck.is_none() && share < 1.0 {
            continue;
        }
        // It falls where it is, an item on the ground.
        transform.translation.y = ground;
        commands.entity(entity).remove::<Flying>();
        if let Some(item) = arsenal.as_deref().and_then(Arsenal::hurled_item) {
            commands.entity(entity).insert((
                Lying {
                    dropped: item.clone(),
                    resting: true,
                },
                models::Sinks,
            ));
        }
    }
}

/// The hurled weapons.
pub struct HurledPlugin;

impl Plugin for HurledPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Dice(crate::seed::dice(1013)))
            .add_systems(
                Update,
                (throw, fly)
                    .chain()
                    .after(Step::Npcs)
                    .in_set(Playing)
                    .in_set(Unpaused),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hit_takes_the_arts_roll_while_the_throwers_clip_plays_and_no_order_came() {
        // Within the clip's last seconds the action is the art's; after its end, none.
        assert!(in_the_art(0.1, 0.3, true, false));
        assert!(!in_the_art(0.3, 0.3, true, false));
        assert!(!in_the_art(0.5, 0.3, true, false));
        // Another action, or the thrower's death, ends it at once.
        assert!(!in_the_art(0.1, 0.3, true, true));
        assert!(!in_the_art(0.1, 0.3, false, false));
    }
}
