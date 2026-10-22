//! The fight as the game plays it (docs/re/fight.md): the creatures' values, health and
//! regeneration as components, the hero's experience and the balance's values as resources, the
//! systems that resolve the hits, regenerate the creatures and raise the hero's level, and the
//! damages' numbers over the targets. The formulas are the rules' (`opensacred_rules::fight`).

use bevy::prelude::*;
use opensacred_data::tables::{Balance, Creature};
pub use opensacred_rules::fight::{
    HERO_LEVEL, Holds, ItemValues, Rank, Templates, combat, hero_health, level, most_health,
    regeneration, skills, takes_on,
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

/// What the hero holds and wears, its template's items (`npcs::Npcs::hero_holds`), a component of
/// the hero's, which its level-up takes.
#[derive(Component, Clone, Debug, Default, PartialEq, Deref)]
pub struct Holding(pub Holds);

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
/// (`Rules::regeneration_rate`), an NPC's `RegInFight`, the hero's `RegInFight` in the fight (a
/// hostile within 350, its stance), `RegEnemyNear` with a hostile within 1,000, else
/// `RegNoEnemy`, times the frame's time in seconds (**hypothesis**), up to its most health; none
/// while it dies.
fn regenerate(
    time: Res<Time>,
    rules: Res<Rules>,
    nearest: Res<crate::crowd::Nearest>,
    mut living: Query<(&mut Health, &mut Regenerates, Has<Body>), Without<Dies>>,
) {
    let hostile = nearest.found.then_some(nearest.distance);
    for (mut health, mut regenerates, hero) in &mut living {
        let rate = rules.regeneration_rate(hero, hostile);
        let gained = regenerates.gather(rate, time.delta_secs());
        if gained > 0 && health.current < health.most {
            health.current = (health.current + gained).min(health.most);
        }
    }
}

/// The hero's level-up (`0x564D60` from `0x57E160`): its attack, defence, damage, resistances,
/// most health and regeneration by its new level, with what it holds and wears, its health
/// refilled.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn level_up(
    experience: Res<Experience>,
    rules: Res<Rules>,
    mut heroes: Query<
        (
            &Base,
            Option<&Holding>,
            &mut Combat,
            &mut Health,
            &mut Regenerates,
        ),
        (With<Body>, Without<Dies>),
    >,
) {
    for (base, holding, mut values, mut health, mut regenerates) in &mut heroes {
        let level = experience.level;
        if health.level == level {
            continue;
        }
        let skills = skills(base, level, true);
        let holds = holding.map(|holding| holding.0.clone()).unwrap_or_default();
        *values = Combat(combat(base, level, holds, &skills, &rules));
        let most = hero_health(base.attributes, level, &skills, &rules);
        *health = Health {
            current: most,
            most,
            level,
        };
        regenerates.value = regeneration(base, level, &skills, &rules);
    }
}

/// How long a damage's number shows, in seconds, and how long of it at full strength before it
/// fades out; how high over the target's feet it starts, in world units, and how fast it rises,
/// in screen pixels a second at zoom 1; its colour over another than the hero and over the hero.
/// As seen in the original (#470): pure yellow and pure red, full for about 0.7 s and gone 1.5 to
/// 2.2 s after the hit, rising about 30 pixels a second; its code is not read.
const NUMBER_SHOWS: f32 = 2.0;
const NUMBER_FULL: f32 = 0.7;
const NUMBER_ABOVE: f32 = 110.0;
const NUMBER_RISES: f32 = 30.0;
const NUMBER_COLOR: Color = Color::srgb(1.0, 1.0, 0.0);
const HERO_HIT: Color = Color::srgb(1.0, 0.0, 0.0);
const NUMBER_SIZE: f32 = 16.0;
/// How wide a number's box is, in logical pixels, centred over the place.
const NUMBER_WIDTH: f32 = 80.0;

/// A damage's number over the place of its target, rising and fading until its time is out.
#[derive(Component)]
struct Number {
    at: Vec3,
    left: f32,
    text: String,
    color: Color,
    label: Option<Entity>,
}

/// How far a number has risen, in world units, and its strength, 1 to 0, with `left` seconds of
/// its time left: full for its first moments, then fading out to its end.
fn shown(left: f32) -> (f32, f32) {
    let risen = crate::iso::height(NUMBER_RISES) * (NUMBER_SHOWS - left);
    (risen, (left / (NUMBER_SHOWS - NUMBER_FULL)).min(1.0))
}

/// Shows each damage's number over its place, rising and, after its first moments, fading, and
/// takes it away when its time is out.
fn rise(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<AssetServer>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    ui_scale: Res<UiScale>,
    mut numbers: Query<(Entity, &mut Number)>,
    mut labels: Query<(&mut Node, &mut TextColor)>,
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
        let (risen, strength) = shown(number.left);
        let Ok(point) = camera.world_to_viewport(view, number.at + Vec3::Y * risen) else {
            continue;
        };
        // The window's pixels in the interface's, which `UiScale` scales (`hud::scale`).
        let point = point / ui_scale.0;
        let (left, top) = (Val::Px(point.x - NUMBER_WIDTH / 2.0), Val::Px(point.y));
        let color = number.color.with_alpha(strength);
        match number.label.map(|label| labels.get_mut(label)) {
            Some(Ok((mut node, mut text))) => {
                node.left = left;
                node.top = top;
                text.0 = color;
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
                        TextColor(color),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_rises_30_pixels_a_second_and_fades_after_its_first_moments() {
        assert_eq!(shown(NUMBER_SHOWS), (0.0, 1.0));
        // Full for 0.7 s, half at 1.35 s, gone at 2 s.
        assert_eq!(shown(NUMBER_SHOWS - NUMBER_FULL).1, 1.0);
        assert!((shown(0.65).1 - 0.5).abs() < 1e-6);
        assert_eq!(shown(0.0).1, 0.0);
        // A second risen: 30 pixels at zoom 1, a world unit's height showing as 1.1 pixels.
        let (risen, _) = shown(NUMBER_SHOWS - 1.0);
        assert!((risen - crate::iso::height(30.0)).abs() < 1e-3 && (27.0..28.0).contains(&risen));
    }
}
