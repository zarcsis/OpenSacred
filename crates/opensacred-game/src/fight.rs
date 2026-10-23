//! The fight as the game plays it (docs/re/fight.md): the creatures' values, health and
//! regeneration as components, the hero's experience and the balance's values as resources, the
//! systems that resolve the hits, regenerate the creatures and raise the hero's level, and the
//! damages' numbers over the targets. The formulas are the rules' (`opensacred_rules::fight`).

use bevy::prelude::*;
use opensacred_data::tables::{Balance, Creature};
pub use opensacred_rules::fight::{
    BOSS_TAG, HERO_LEVEL, Holds, ItemValues, Raise, Rank, Templates, combat, fill_level,
    filled_level, hero_health, most_health, regeneration, scripted_level, skills, tagged_level,
    takes_on,
};
use opensacred_rules::fight::{
    Side, art_strike, experience_of, range_factor, strike, weapons_factor, weighed,
};

use crate::game::{AppState, FONT};
use crate::hero::Body;
use crate::npcs::Dies;
use crate::spawning::Rng;

/// A hit of a strike (`0x4EA3F0`, which the strike `0x4EA1F0` calls as it lands, the event 5):
/// its attacker, its target and its share of the strike, 0.5 for each hit of two (docs/re/fight.md,
/// "Melee: the AI's state 4", "A strike's resolution"), whether it is a hit at range, an
/// arrow's (`arrows`), with the bow in its attacker's hand (`0x5BB010`), and for a strike of a
/// combat art of the second table its level in play, its factor the share (`making`, #509).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub attacker: Entity,
    pub target: Entity,
    pub share: f32,
    pub ranged: bool,
    pub art: Option<u32>,
}

/// The dice of the strikes.
#[derive(Resource)]
pub(crate) struct Dice(Rng);

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

/// A creature that takes no damage, for a test (the commands `spawn --immortal` and `god`,
/// #521): a hit lands and shows its damage, but the health stays.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Immortal;

/// Resolves each hit (`0x56B130`), weighed by the difficulty between the hero and a creature
/// ([`weighed`]), a hit at range with its attack times the range's factor of the distance between
/// the attacker and the target ([`range_factor`], `0x56B449`), and a creature's at the hero, but a
/// boss's or a champion's, with its damage times the factor of its level ([`weapons_factor`],
/// `0x56A890`): a target without a health, or dying, ignores it; else the strike may miss, and
/// its damage takes the target's health down, shows over it, and at 0 the target dies (the AI's
/// state 9, `0x53E7C0`), one of its two dying clips at random, and a hero that killed it earns
/// its experience (the event 9, `0x57D860`).
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
        Option<&Ranked>,
    )>,
    mut targets: Query<
        (
            &mut Health,
            &Combat,
            &GlobalTransform,
            Option<&Worth>,
            Has<Body>,
            Option<&Ranked>,
            Has<Immortal>,
        ),
        Without<Dies>,
    >,
) {
    for hit in hits.read() {
        let (
            Ok((attacker, hero, mind, from, ranked)),
            Ok((mut health, defender, at, worth, hero_hit, rank, immortal)),
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
        let (mut attacker, defender, mut blow) =
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
        let ranked = ranked.map_or(Rank::Common, |ranked| ranked.0);
        let factor = weapons_factor(side, level, ranked, &rules);
        blow.share = hit.share;
        blow.weapons = factor;
        if side == Some(Side::Defender) {
            debug!(
                "fight: {} of level {level} strikes {}, its damage times {factor:.4}",
                hit.attacker, hit.target
            );
        }
        let landed = match hit.art {
            Some(art) => art_strike(&attacker, &defender, blow, art, &mut dice.0),
            None => strike(&attacker, &defender, blow, &mut dice.0),
        };
        let Some(damage) = landed else {
            debug!("fight: {} misses {}", hit.attacker, hit.target);
            continue;
        };
        if !immortal {
            health.current = health.current.saturating_sub(damage);
        }
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
            let worth = worth.filter(|_| hero).map(|worth| (*worth, rank.copied()));
            fell(
                &mut commands,
                (&mut dice, &mut experience, &rules),
                hit.target,
                &health,
                worth,
            );
        }
    }
}

/// Fells `target`, at `health`: it dies, one of its two dying clips at random, and where the
/// hero killed it, of `worth` and its rank, the hero earns its experience (the event 9,
/// `0x57D860`), twice for a boss and 1.5 times for a champion; the experience earned.
fn fell(
    commands: &mut Commands,
    (dice, experience, rules): (&mut Dice, &mut Experience, &Rules),
    target: Entity,
    health: &Health,
    worth: Option<(Worth, Option<Ranked>)>,
) -> u32 {
    commands.entity(target).insert(Dies::new(dice.0.below(2)));
    let Some((worth, rank)) = worth else {
        return 0;
    };
    let earned = experience_of(worth.0, health.level, experience.level, rules);
    let earned = rank.unwrap_or_default().0.earned(earned);
    let grew = experience.add(u64::from(earned), rules);
    info!(
        "fight: the hero kills {target} and earns {earned} experience, {} in all",
        experience.points
    );
    if grew {
        info!("fight: the hero reaches level {}", experience.level);
    }
    earned
}

/// Fells the hostile creatures within `radius` cells of the hero along each axis as if the hero
/// killed them (the command `kill`, #524): they die, drop and give the hero their experience; a
/// line for each.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
pub fn kill(
    In(radius): In<u32>,
    mut commands: Commands,
    mut dice: ResMut<Dice>,
    mut experience: ResMut<Experience>,
    rules: Res<Rules>,
    hero: Query<&Transform, With<Body>>,
    mut creatures: Query<
        (
            Entity,
            &Transform,
            &mut Health,
            &crate::ai::Mind,
            Option<&Worth>,
            Option<&Ranked>,
        ),
        (Without<Dies>, Without<Body>),
    >,
) -> Result<Vec<String>, String> {
    let at = hero
        .iter()
        .next()
        .ok_or("the hero is not in the world yet")?
        .translation;
    let home = crate::paths::cell_of(at.x, at.z);
    let radius = i32::try_from(radius).unwrap_or(i32::MAX);
    let mut lines = Vec::new();
    for (entity, transform, mut health, mind, worth, rank) in &mut creatures {
        let here = transform.translation;
        let cell = crate::paths::cell_of(here.x, here.z);
        if !mind.hostile() || (cell.0 - home.0).abs() > radius || (cell.1 - home.1).abs() > radius {
            continue;
        }
        health.current = 0;
        let worth = worth.map(|worth| (*worth, rank.copied()));
        let earned = fell(
            &mut commands,
            (&mut dice, &mut experience, &rules),
            entity,
            &health,
            worth,
        );
        lines.push(format!(
            "{entity} type {} level {} falls, {earned} experience",
            mind.creature, health.level
        ));
    }
    lines.sort();
    lines.insert(
        0,
        format!(
            "{} hostile creatures fall; the hero has {} experience, level {}",
            lines.len(),
            experience.points,
            experience.level
        ),
    );
    Ok(lines)
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

/// The regeneration (`0x562730`), on the world's tick as the creatures' updates run it
/// (`frame::WorldTick`, #616): each creature's value times its rate
/// (`Rules::regeneration_rate`), an NPC's `RegInFight`, the hero's `RegInFight` in the fight (a
/// hostile within 350, its stance), `RegEnemyNear` with a hostile within 1,000, else
/// `RegNoEnemy`, times the tick's seconds, up to its most health; none while it dies.
fn regenerate(
    tick: Res<crate::frame::WorldTick>,
    rules: Res<Rules>,
    nearest: Res<crate::crowd::Nearest>,
    mut living: Query<(&mut Health, &mut Regenerates, Has<Body>), Without<Dies>>,
) {
    let Some(seconds) = tick.seconds else {
        return;
    };
    let hostile = nearest.found.then_some(nearest.distance);
    for (mut health, mut regenerates, hero) in &mut living {
        let rate = rules.regeneration_rate(hero, hostile);
        let full = health.current == 0 || health.current >= health.most;
        let gained = regenerates.gather(rate, seconds, full);
        if gained > 0 {
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
            .add_systems(
                Update,
                regenerate
                    .in_set(crate::frame::Unpaused)
                    .after(crate::frame::Step::Tick)
                    .run_if(crate::frame::ticked),
            );
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
    fn an_immortal_takes_the_hits_but_keeps_its_health() {
        let combat = Combat(opensacred_rules::fight::Combat {
            attack: 200.0,
            defence: 10.0,
            damage: [[20.0, 30.0], [0.0; 2], [0.0; 2], [0.0; 2]],
            resistance: [0.0; 4],
            damping: [0; 4],
            speed: 140.0,
            pace: 130.0,
        });
        let health = Health {
            current: 1_000_000,
            most: 1_000_000,
            level: 1,
        };
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<Hit>()
            .insert_resource(Dice(Rng::of(0, 0, 0)))
            .init_resource::<Experience>()
            .init_resource::<Rules>()
            .add_systems(Update, resolve);
        let world = app.world_mut();
        let hero = world.spawn((combat, GlobalTransform::default())).id();
        let mortal = world
            .spawn((combat, health, GlobalTransform::default()))
            .id();
        let immortal = world
            .spawn((combat, health, GlobalTransform::default(), Immortal))
            .id();
        for target in [mortal, immortal] {
            for _ in 0..50 {
                world.write_message(Hit {
                    attacker: hero,
                    target,
                    share: 1.0,
                    ranged: false,
                    art: None,
                });
            }
        }
        app.update();
        let current = |entity| app.world().get::<Health>(entity).unwrap().current;
        assert!(current(mortal) < health.current);
        assert_eq!(current(immortal), health.current);
    }

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
