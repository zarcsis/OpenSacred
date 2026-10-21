//! The fight as the game plays it (docs/re/fight.md): the creatures' values, health and
//! regeneration as components, the hero's experience and the balance's values as resources, the
//! systems that resolve the hits, regenerate the creatures and raise the hero's level, and the
//! damages' numbers over the targets. The formulas are the rules' (`opensacred_rules::fight`).

use bevy::prelude::*;
use opensacred_data::tables::{Balance, Creature};
pub use opensacred_rules::fight::{
    HERO_LEVEL, Holds, Rank, Templates, combat, hero_health, level, most_health, regeneration,
    skills, takes_on,
};
use opensacred_rules::fight::{Side, experience_of, range_factor, strike, weighed};

use crate::game::{AppState, FONT};
use crate::hero::Body;
use crate::npcs::Dies;
use crate::spawning::Rng;

/// A hit of a strike (`0x4EA3F0`, which the strike `0x4EA1F0` calls as it lands, the event 5):
/// its attacker, its target and its share of the strike, 0.5 for each hit of two (docs/re/fight.md,
/// "Melee: the AI's state 4", "A strike's resolution"), and whether it is a hit at range, an
/// arrow's (`arrows`), with the bow in its attacker's hand (`0x5BB010`).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub attacker: Entity,
    pub target: Entity,
    pub share: f32,
    pub ranged: bool,
}

/// The dice of the strikes.
#[derive(Resource)]
struct Dice(Rng);

/// The hero's experience, its level and the points its levels gave
/// (`opensacred_rules::fight::Experience`).
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq, Deref, DerefMut)]
pub struct Experience(pub opensacred_rules::fight::Experience);

/// The balance's values that the fight takes (`opensacred_rules::fight::Rules`).
#[derive(Resource, Clone, Debug, Default, PartialEq, Deref)]
pub struct Rules(pub opensacred_rules::fight::Rules);

impl Rules {
    /// The rules of the balance `balance`, the original's values where it lacks them.
    pub fn of(balance: Option<&Balance>) -> Self {
        Self(opensacred_rules::fight::Rules::of(balance))
    }
}

/// A creature type's values of `Creature.pak` that its fight takes
/// (`opensacred_rules::fight::Base`), a component of the hero's, which its level-up takes.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Deref)]
pub struct Base(pub opensacred_rules::fight::Base);

impl Base {
    pub fn of(creature: &Creature) -> Self {
        Self(opensacred_rules::fight::Base::of(creature))
    }
}

/// A creature's attack, defence, damage and resistances and its attack speed
/// (`opensacred_rules::fight::Combat`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Deref)]
pub struct Combat(pub opensacred_rules::fight::Combat);

/// A creature's rank, a fill's boss or champion (`opensacred_rules::fight::Rank`).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ranked(pub Rank);

/// What a creature is worth: `Creature.pak`'s experience `a + E·b` (`+0x08`, `+0x0A`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Worth(pub [u16; 2]);

/// Resolves each hit (`0x56B130`), weighed by the difficulty between the hero and a creature
/// ([`weighed`]), a hit at range with its attack times the range's factor of the distance between
/// the attacker and the target ([`range_factor`], `0x56B449`): a target without a health, or
/// dying, ignores it; else the strike may miss, and its damage takes the target's health down,
/// shows over it, and at 0 the target dies (the AI's state 9, `0x53E7C0`), one of its two dying
/// clips at random, and a hero that killed it earns its experience (the event 9, `0x57D860`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn resolve(
    mut commands: Commands,
    mut hits: MessageReader<Hit>,
    mut dice: ResMut<Dice>,
    mut experience: ResMut<Experience>,
    rules: Res<Rules>,
    attackers: Query<(
        &Combat,
        Has<Body>,
        Option<&crate::ai::Mind>,
        &GlobalTransform,
    )>,
    mut targets: Query<
        (
            &mut Health,
            &Combat,
            &GlobalTransform,
            Option<&Worth>,
            Has<Body>,
            Option<&Ranked>,
        ),
        Without<Dies>,
    >,
) {
    for hit in hits.read() {
        let (
            Ok((attacker, hero, mind, from)),
            Ok((mut health, defender, at, worth, hero_hit, rank)),
        ) = (attackers.get(hit.attacker), targets.get_mut(hit.target))
        else {
            debug!(
                "fight: {} hits {} ({:.1}), which takes no damage yet",
                hit.attacker, hit.target, hit.share
            );
            continue;
        };
        if health.current == 0 {
            continue;
        }
        let side = match (hero, hero_hit) {
            (true, false) => Some(Side::Attacker),
            (false, true) => Some(Side::Defender),
            _ => None,
        };
        let level = mind.map_or(experience.level, |mind| mind.level);
        let (mut attacker, defender, k) =
            weighed(attacker.0, defender.0, side, (level, health.level), &rules);
        if hit.ranged {
            let distance = (from.translation() - at.translation()).xz().length();
            let factor = range_factor(distance, &rules);
            attacker.attack *= factor;
            debug!(
                "fight: {} at range {distance:.0} of {}, its attack times {factor:.2}",
                hit.attacker, hit.target
            );
        }
        let Some(damage) = strike(&attacker, &defender, (hit.share, k), &mut dice.0) else {
            debug!("fight: {} misses {}", hit.attacker, hit.target);
            continue;
        };
        health.current = health.current.saturating_sub(damage);
        debug!(
            "fight: {} hits {} for {damage}, health {}/{}",
            hit.attacker, hit.target, health.current, health.most
        );
        commands.spawn((
            Number {
                at: at.translation() + Vec3::Y * NUMBER_ABOVE,
                left: NUMBER_SHOWS,
                text: damage.to_string(),
                color: if hero_hit { HERO_HIT } else { NUMBER_COLOR },
                label: None,
            },
            DespawnOnExit(AppState::InGame),
        ));
        if health.current == 0 {
            commands
                .entity(hit.target)
                .insert(Dies::new(dice.0.below(2)));
            if let (true, Some(worth)) = (hero, worth) {
                let earned = experience_of(worth.0, health.level, experience.level, &rules);
                // Twice for a boss, 1.5 times for a champion.
                let earned = rank.copied().unwrap_or_default().0.earned(earned);
                let grew = experience.add(u64::from(earned), &rules);
                info!(
                    "fight: the hero kills {} and earns {earned} experience, {} in all",
                    hit.target, experience.points
                );
                if grew {
                    info!("fight: the hero reaches level {}", experience.level);
                }
            }
        }
    }
}

/// A creature's regeneration (`opensacred_rules::fight::Regenerates`): its value and what it has
/// gathered toward its next health.
#[derive(Component, Clone, Copy, Debug, PartialEq, Deref, DerefMut)]
pub struct Regenerates(pub opensacred_rules::fight::Regenerates);

impl Regenerates {
    pub fn new(value: f32) -> Self {
        Self(opensacred_rules::fight::Regenerates::new(value))
    }
}

/// The regeneration (`0x562730`): each creature's value times its rate
/// (`Rules::regeneration_rate`), an NPC's `RegInFight`, the hero's `RegInFight` while it attacks,
/// `RegEnemyNear` with a hostile within 1,000, else `RegNoEnemy`, times the frame's time in
/// seconds (**hypothesis**), up to its most health; none while it dies.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn regenerate(
    time: Res<Time>,
    rules: Res<Rules>,
    nearest: Res<crate::crowd::Nearest>,
    mut living: Query<
        (
            &mut Health,
            &mut Regenerates,
            Has<Body>,
            Has<crate::attack::Attack>,
        ),
        Without<Dies>,
    >,
) {
    let hostile = nearest.found.then_some(nearest.distance);
    for (mut health, mut regenerates, hero, attacks) in &mut living {
        let rate = rules.regeneration_rate(hero, attacks, hostile);
        let gained = regenerates.gather(rate, time.delta_secs());
        if gained > 0 && health.current < health.most {
            health.current = (health.current + gained).min(health.most);
        }
    }
}

/// The hero's level-up (`0x564D60` from `0x57E160`): its attack, defence, damage, resistances,
/// most health and regeneration by its new level, its health refilled.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn level_up(
    experience: Res<Experience>,
    rules: Res<Rules>,
    mut heroes: Query<
        (&Base, &mut Combat, &mut Health, &mut Regenerates),
        (With<Body>, Without<Dies>),
    >,
) {
    for (base, mut values, mut health, mut regenerates) in &mut heroes {
        let level = experience.level;
        if health.level == level {
            continue;
        }
        let skills = skills(base, level, true);
        *values = Combat(combat(base, level, Holds::default(), &skills, &rules));
        let most = hero_health(base.attributes, level);
        *health = Health {
            current: most,
            most,
            level,
        };
        regenerates.value = regeneration(base, level, &skills, &rules);
    }
}

/// How long a damage's number shows, in seconds, how high over the target's feet it starts and
/// how far it rises meanwhile, in world units, and its colour (**hypothesis**: the original's
/// number is not read).
const NUMBER_SHOWS: f32 = 1.0;
const NUMBER_ABOVE: f32 = 110.0;
const NUMBER_RISES: f32 = 60.0;
const NUMBER_COLOR: Color = Color::srgb(1.0, 0.85, 0.3);
const HERO_HIT: Color = Color::srgb(1.0, 0.3, 0.25);
const NUMBER_SIZE: f32 = 16.0;
/// How wide a number's box is, in logical pixels, centred over the place.
const NUMBER_WIDTH: f32 = 80.0;

/// A damage's number over the place of its target, rising until its time is out.
#[derive(Component)]
struct Number {
    at: Vec3,
    left: f32,
    text: String,
    color: Color,
    label: Option<Entity>,
}

/// Shows each damage's number over its place, rising, and takes it away when its time is out.
fn rise(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<AssetServer>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    ui_scale: Res<UiScale>,
    mut numbers: Query<(Entity, &mut Number)>,
    mut nodes: Query<&mut Node>,
) {
    let Some((camera, view)) = cameras.iter().next() else {
        return;
    };
    for (entity, mut number) in &mut numbers {
        number.left -= time.delta_secs();
        if number.left <= 0.0 {
            if let Some(label) = number.label {
                commands.entity(label).despawn();
            }
            commands.entity(entity).despawn();
            continue;
        }
        let risen = NUMBER_RISES * (1.0 - number.left / NUMBER_SHOWS);
        let Ok(point) = camera.world_to_viewport(view, number.at + Vec3::Y * risen) else {
            continue;
        };
        // The window's pixels in the interface's, which `UiScale` scales (`hud::scale`).
        let point = point / ui_scale.0;
        let (left, top) = (Val::Px(point.x - NUMBER_WIDTH / 2.0), Val::Px(point.y));
        match number.label.map(|label| nodes.get_mut(label)) {
            Some(Ok(mut node)) => {
                node.left = left;
                node.top = top;
            }
            _ => {
                let label = commands
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left,
                            top,
                            width: Val::Px(NUMBER_WIDTH),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        Text::new(number.text.clone()),
                        TextFont {
                            font: assets.load(FONT).into(),
                            font_size: FontSize::Px(NUMBER_SIZE),
                            ..default()
                        },
                        TextColor(number.color),
                        TextLayout::justify(Justify::Center),
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id();
                number.label = Some(label);
            }
        }
    }
}

/// The fight's rules.
pub struct FightPlugin;

impl Plugin for FightPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Hit>()
            .insert_resource(Dice(Rng::of(0, 0, 0)))
            .init_resource::<Experience>()
            .init_resource::<Rules>()
            .add_systems(
                OnEnter(AppState::InGame),
                |mut experience: ResMut<Experience>| *experience = Experience::default(),
            )
            .add_systems(Update, (resolve, level_up, rise).chain())
            .add_systems(Update, regenerate.in_set(crate::frame::Unpaused));
    }
}

/// A creature's health, its most, and its level.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Health {
    pub current: u32,
    pub most: u32,
    pub level: u32,
}
