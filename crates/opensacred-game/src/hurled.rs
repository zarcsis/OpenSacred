//! The Gladiator's hurled weapon (1013, #755; docs/re/arts.md, "The Gladiator's"). As below 0.6
//! of the art's clip is left (`making`), the weapon in his left hand, else his right, leaves it
//! (`0x556A30`, his arms then without it, `arsenal`) and flies on an arc from him to 200 beyond
//! his target along the line to it (`0x501D39`), 80 high, for the ground distance to the target
//! over 1,000 seconds (`0x44DFE0`'s arc branch). It strikes the first creature that it touches
//! by the sum of their sizes, alive and not the hero (`0x600340`), once, with its own damages,
//! each rolled between its least and most and times `1 + c(s₂)/100 + a0/BalanceGeschick`, and
//! the art's factor (`0x56B130` at `0x56BC81`); then it falls there, or at its end, an item lying
//! on the ground, which the hero picks up again into his hand (`pickup`).

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
/// stood, the art's level in play and its factor.
#[derive(Component, Clone, Copy, Debug)]
pub struct Hurls {
    pub at: Vec3,
    pub level: u32,
    pub factor: f32,
}

/// A weapon in flight: its arc's ends on the ground, its height, its time and the time flown, the
/// level it flies on, its thrower, the art's level and factor, and its size.
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
    size: f32,
}

/// The dice of the hurled weapons' damages.
#[derive(Resource)]
struct Dice(Rng);

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
        let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(hurled.held.model.clone()));
        let mut entity = commands.spawn((
            Transform::from_translation(transform.translation)
                .with_scale(Vec3::splat(iso::MODEL_SCALE)),
            WorldAssetRoot(scene),
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
    throwers: Query<&Combat>,
    creatures: Query<
        (
            Entity,
            &GlobalTransform,
            &Health,
            Option<&crate::arrows::Size>,
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
        let struck = creatures
            .iter()
            .filter(|(.., health, _)| health.current > 0)
            .find(|(_, place, _, size)| {
                let size = size.map_or(crate::arrows::size(0), |size| size.0);
                (place.translation().xz() - at).length() <= flight.size + size
            })
            .map(|(target, ..)| target);
        if let (Some(target), Some(item)) = (
            struck,
            arsenal
                .as_deref()
                .and_then(|arsenal| arsenal.hurled_item())
                .and_then(|item| item.values),
        ) {
            // Its own damages, each rolled at the hundredth between its least and most, times
            // `1 + c(s₂)/100 + a0/BalanceGeschick` (`0x56ACD0`, as 1016's).
            let strength = throwers
                .get(flight.thrower)
                .map_or(0.0, |combat| combat.attributes[0]);
            let skills = arts
                .as_deref()
                .map(|arts| arts.skills.clone())
                .unwrap_or_default();
            let times = rules.glance_factor(&skills, strength);
            let fixed = arts::thrown_damage(&item.damage, times, &mut dice.0);
            info!(
                "hurled: the hurled weapon strikes {target}, {:?}",
                fixed.map(|d| d.round())
            );
            hits.write(fight::Hit {
                attacker: flight.thrower,
                target,
                share: flight.art.1,
                ranged: false,
                art: Some(flight.art.0),
                fixed: Some(fixed),
                // The art over at the throw, the hit takes no roll (the kind 3 never takes the
                // plain one).
                sure: true,
                poison: false,
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
                    dropped: *item,
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
        app.insert_resource(Dice(Rng::of(0, 0, 1013))).add_systems(
            Update,
            (throw, fly)
                .chain()
                .after(Step::Npcs)
                .in_set(Playing)
                .in_set(Unpaused),
        );
    }
}
