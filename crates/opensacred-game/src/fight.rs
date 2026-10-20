//! The fight's rules as far as the game follows them (docs/re/fight.md): the creatures' levels,
//! their health, their attack, defence, damage and resistances, and a strike's hit.

use bevy::prelude::*;
use opensacred_data::tables::{Balance, Creature};

use crate::game::{AppState, FONT};
use crate::hero::Body;
use crate::npcs::Dies;
use crate::spawning::{Rng, SpawnValues};

/// The hero's level, which the creatures' levels follow: the hero gains none yet.
pub const HERO_LEVEL: u32 = 1;
/// The balance's row of a difficulty's values that the game plays: bronze, the row's sixth
/// value (docs/re/fight.md).
const BRONZE: usize = 5;

/// A hit of a strike (`0x4EA3F0`, the event 5): its attacker, its target and its share of the
/// strike, 0.5 for each hit of two (docs/re/fight.md, "A strike's resolution").
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub attacker: Entity,
    pub target: Entity,
    pub share: f32,
}

/// The kinds of damage: physical, fire, magic and poison.
const KINDS: usize = 4;

/// The rows of the table of `A` and `D` at `0x956C80`, all alike: the per cent of the attributes
/// `a0` and `a2` that the attack takes, and that the defence takes (docs/re/fight.md, "An NPC's
/// attack, defence and damage").
const ATTACK: [f32; 2] = [50.0, 50.0];
const DEFENCE: [f32; 2] = [20.0, 80.0];

/// The dice of the strikes.
#[derive(Resource)]
struct Dice(Rng);

/// The hero's experience, which its kills earn (`0x57D860`), its level by it (`0x57F8D0`), and
/// the attribute and skill points its levels gave (`0x57E160`), which it cannot spend yet, and
/// the skills it may still learn.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Experience {
    pub points: u64,
    pub level: u32,
    pub attribute_points: u32,
    pub skill_points: u32,
    to_learn: u32,
}

impl Default for Experience {
    fn default() -> Self {
        Self {
            points: 0,
            level: HERO_LEVEL,
            attribute_points: 0,
            skill_points: 0,
            to_learn: 0,
        }
    }
}

/// The skill slots a hero holds at the start (**hypothesis**, docs/re/fight.md).
const HELD_SKILLS: u32 = 2;
/// The highest level (`0x57F8D0`).
const TOP_LEVEL: u32 = 216;

impl Experience {
    /// Adds `earned` (`0x57E160`), the experience held below the top level's end, and gives the
    /// levels it reaches: for each, an attribute point, and for each level `p` reached, half,
    /// rounded up, of the skill slots held and open (two held, one more open from each of the
    /// balance's `skilllearn` levels up to `p`); returns whether the level grew.
    pub fn add(&mut self, earned: u64, rules: &Rules) -> bool {
        self.points = (self.points + earned).min(level_end(TOP_LEVEL) - 1);
        let reached = level_of(self.points, self.level);
        if reached <= self.level {
            return false;
        }
        let from = self.level;
        for p in from + 1..=reached {
            let opened = rules
                .skill_levels
                .iter()
                .filter(|&&t| from < t && t <= p)
                .count();
            let n = HELD_SKILLS + self.to_learn + u32::try_from(opened).unwrap_or(0);
            self.skill_points += n.div_ceil(2);
        }
        self.attribute_points += reached - from;
        let opened = rules
            .skill_levels
            .iter()
            .filter(|&&t| from < t && t <= reached)
            .count();
        self.to_learn += u32::try_from(opened).unwrap_or(0);
        self.level = reached;
        true
    }
}

/// The experience at which level `level` ends (`0x57F6E0`):
/// `⌊(L⁴ + 40L³ + 150L² + 200L − 90)/100⌋·100`.
pub fn level_end(level: u32) -> u64 {
    let l = u64::from(level);
    (l.pow(4) + 40 * l.pow(3) + 150 * l.pow(2) + 200 * l).saturating_sub(90) / 100 * 100
}

/// The level of `points` of experience (`0x57F8D0`): the one whose span holds them, at most 216,
/// the level `current` while they are below the first level's end.
pub fn level_of(points: u64, current: u32) -> u32 {
    if points < level_end(1) {
        return current;
    }
    (1..TOP_LEVEL)
        .take_while(|&level| level_end(level) <= points)
        .last()
        .map_or(current, |level| level + 1)
}

/// What a creature is worth: `Creature.pak`'s experience `a + E·b` (`+0x08`, `+0x0A`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Worth(pub [u16; 2]);

/// Resolves each hit (`0x56B130`), weighed by the difficulty between the hero and a creature
/// ([`weighed`]): a target without a health, or dying, ignores it; else the strike may miss, and its damage takes the target's health down,
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
    attackers: Query<(&Combat, Has<Body>, Option<&crate::ai::Mind>)>,
    mut targets: Query<
        (
            &mut Health,
            &Combat,
            &GlobalTransform,
            Option<&Worth>,
            Has<Body>,
        ),
        Without<Dies>,
    >,
) {
    for hit in hits.read() {
        let (Ok((attacker, hero, mind)), Ok((mut health, defender, at, worth, hero_hit))) =
            (attackers.get(hit.attacker), targets.get_mut(hit.target))
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
        let (attacker, defender, k) =
            weighed(*attacker, *defender, side, (level, health.level), &rules);
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

/// The experience that a hero of level `hero` earns for a creature of level `level` worth
/// `[a, b]` (`0x57D860`): `a + E·b`, `E` the level on bronze's scale, the game's, times bronze's
/// `ProzExp`, times the factor of the levels (`0x551530`): with `r = (E + 9)/(H + 9)` against
/// the threshold `t` of the test of the levels (`takes_on`), 0.04 up to `t`, then
/// `(r − t)/(UnterExpVoll − t)`, 1 from `UnterExpVoll` to `ObereEXPVoll`, `ObereEXPVoll/r` above,
/// at least 0.04, and 1 for none; above bronze's `LevelKap` the hero's level `H` takes it down by
/// `(LevelKap/H)²`, from level 100 the decay of a high hero (`0x57E050`), and above the bronze
/// levels the fade of a difficulty outgrown (`StartLevMulti`); at least 1, whole. No bonuses of
/// the killer, bosses or champions yet (docs/decisions/2026-10-20-levels.md).
pub fn experience_of([a, b]: [u16; 2], level: u32, hero: u32, rules: &Rules) -> u32 {
    let [share, anteil, voll, upper] = rules.experience;
    let (e, h) = (to_f32(level), to_f32(hero));
    let r = (e + 9.0) / (h + 9.0);
    let t = match hero {
        0..=10 => (h + 6.0) / (h + 9.0),
        92.. => (h - 6.0) / (h + 9.0),
        _ => anteil,
    };
    let factor = if r <= t {
        0.04
    } else if r > upper {
        upper / r
    } else if r >= voll {
        1.0
    } else {
        (r - t) / (voll - t)
    };
    let mut x = (f32::from(a) + e * f32::from(b)) * share;
    let cap = rules.level_cap;
    if cap > 0.0 && h > cap {
        x *= cap * cap / (h * h);
    }
    x *= factor.max(0.04);
    if x <= 0.0 {
        x = 1.0;
    }
    x *= decay(hero) * fade(h, rules.start_levels);
    whole(x.max(1.0))
}

/// The decay of the experience of a hero of level `hero` (`0x57E050`): 1 up to level 99,
/// `0.985^(H − 99)` to 124, `0.98^(H − 99)` to 149, `0.975^(H − 109)` from 150.
fn decay(hero: u32) -> f32 {
    let power = |base: f32, from: u32| base.powf(to_f32(hero.saturating_sub(from)));
    match hero {
        0..=99 => 1.0,
        100..=124 => power(0.985, 99),
        125..=149 => power(0.98, 99),
        _ => power(0.975, 109),
    }
}

/// The fade of a difficulty outgrown, on bronze (`0x57D860`): 1 up to `S1 + 10`, then
/// `(S2 + 1 − H)/(S2 − S1 − 9)`, at least 0, of the row `StartLevMulti`'s `S1` and `S2`.
fn fade(h: f32, [s1, s2]: [f32; 2]) -> f32 {
    if h <= s1 + 10.0 {
        return 1.0;
    }
    ((s2 + 1.0 - h) / (s2 - s1 - 9.0)).max(0.0)
}

/// A creature's regeneration (`0x562730`): its value (`+0x138`) and what it has gathered toward
/// its next health.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Regenerates {
    pub value: f32,
    kept: f32,
}

impl Regenerates {
    pub fn new(value: f32) -> Self {
        Self { value, kept: 0.0 }
    }

    /// Gathers `value · rate · seconds` and gives the health it makes: each 4,096 one, the
    /// remainder kept the low ten bits of what was gathered when it gives any (the original's
    /// mask `0x3FF`, not `0xFFF`).
    fn gather(&mut self, rate: f32, seconds: f32) -> u32 {
        self.kept += self.value * rate * seconds;
        let gathered = whole(self.kept);
        if gathered < 4096 {
            return 0;
        }
        self.kept = to_f32(gathered & 0x3FF);
        gathered >> 12
    }
}

/// The regeneration value of a creature of `base` at level `level` (`0x5796A0`): `2·a3`, its
/// attribute 3 raised by its level, times 1 plus its skill 10's per cent (`0x566070`).
pub fn regeneration(base: &Base, level: u32, skills: &[(u8, u32)], rules: &Rules) -> f32 {
    let b3 = u32::from(base.attributes[3]);
    let value = to_f32(2 * (b3 + b3 * (level.max(1) - 1) / 10));
    let skill: f32 = skills
        .iter()
        .filter(|&&(skill, _)| skill == 10)
        .map(|&(_, at)| curve(rules.curves.regeneration, at) / 100.0)
        .sum();
    value * (1.0 + skill)
}

/// How near a hostile makes the hero regenerate at `RegEnemyNear` (`0x562730`).
const ENEMY_NEAR: f32 = 1000.0;

/// The regeneration (`0x562730`): each creature's value times its rate, an NPC's `RegInFight`,
/// the hero's `RegInFight` while it attacks, `RegEnemyNear` with a hostile within 1,000, else
/// `RegNoEnemy`, times the frame's time in seconds (**hypothesis**), up to its most health; none
/// while it dies.
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
    let [fighting, near, alone] = rules.regeneration;
    for (mut health, mut regenerates, hero, attacks) in &mut living {
        let rate = if !hero || attacks {
            fighting
        } else if nearest.found && nearest.distance < ENEMY_NEAR {
            near
        } else {
            alone
        };
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
        *values = combat(base, level, Holds::default(), &skills, &rules);
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

/// The balance's values that a creature's fight takes: the points of the level's factor of the
/// health of a creature not a hero (`HP_s1`…`HP_w5`) and the difficulty's `ProzHP`; the offset
/// of the level's factor of the attributes (`BalStatOff`), the divisors of each kind's damage
/// and resistance multipliers (`BalChar*D`, `BalChar*R`) and of the resistances
/// (`BalanceRes*`), and the factors of all damages and resistances (`BalanceDmg`, `BalanceRes`);
/// the experience's factor of bronze (`ProzExp`) and the levels' (`UnterExpAnteil`,
/// `UnterExpVoll`, `ObereEXPVoll`); bronze's factors of the strikes between the hero and the
/// creatures (`ProzAW`, `ProzDmg`, `ProzRes`); and the hero's rates of regeneration fighting,
/// with a hostile near and without (`RegInFight`, `RegEnemyNear`, `RegNoEnemy`).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Rules {
    points: Vec<(f32, f32)>,
    factor: f32,
    stat_offset: f32,
    damage_divisors: [f32; KINDS],
    resistance_divisors: [f32; KINDS],
    resistances: [f32; KINDS],
    balance: [f32; 2],
    experience: [f32; 4],
    difficulty: [f32; 3],
    regeneration: [f32; 3],
    curves: Curves,
    level_cap: f32,
    start_levels: [f32; 2],
    skill_levels: [u32; 6],
}

impl Default for Rules {
    /// Without the balance, no factor of the health, and the original's values of the rest.
    fn default() -> Self {
        Self {
            points: Vec::new(),
            factor: 1.0,
            stat_offset: 20.0,
            damage_divisors: [220.0, 440.0, 220.0, 440.0],
            resistance_divisors: [280.0, 280.0, 320.0, 260.0],
            resistances: [22.0, 17.0, 17.0, 25.0],
            balance: [1.0, 1.0],
            experience: [1.0, 0.85, 1.0, 1.1],
            difficulty: [0.65, 0.9, 1.0],
            regeneration: [36.0, 48.0, 128.0],
            curves: Curves::default(),
            level_cap: 50.0,
            start_levels: [60.0, 100.0],
            skill_levels: [3, 6, 12, 20, 30, 50],
        }
    }
}

impl Rules {
    /// The rules of the balance `balance`, the original's values where it lacks them (the
    /// health's factors 1).
    pub fn of(balance: Option<&Balance>) -> Self {
        let default = Self::default();
        let Some(b) = balance else {
            return default;
        };
        let points = (1..=5)
            .map(|i| Some((b.f32(&format!("HP_s{i}"))?, b.f32(&format!("HP_w{i}"))?)))
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default();
        let named = |names: [&str; KINDS], or: [f32; KINDS]| {
            let mut values = or;
            for (value, name) in values.iter_mut().zip(names) {
                *value = b.f32(name).unwrap_or(*value);
            }
            values
        };
        Self {
            points,
            factor: b.row_f32("ProzHP", BRONZE).unwrap_or(1.0),
            stat_offset: b.f32("BalStatOff").unwrap_or(default.stat_offset),
            damage_divisors: named(
                ["BalCharPD", "BalCharFD", "BalCharMD", "BalCharGD"],
                default.damage_divisors,
            ),
            resistance_divisors: named(
                ["BalCharPR", "BalCharFR", "BalCharMR", "BalCharGR"],
                default.resistance_divisors,
            ),
            resistances: named(
                [
                    "BalanceResPh",
                    "BalanceResFe",
                    "BalanceResMa",
                    "BalanceResGi",
                ],
                default.resistances,
            ),
            balance: [
                b.f32("BalanceDmg").unwrap_or(1.0),
                b.f32("BalanceRes").unwrap_or(1.0),
            ],
            experience: [
                b.row_f32("ProzExp", BRONZE).unwrap_or(1.0),
                b.f32("UnterExpAnteil").unwrap_or(0.85),
                b.f32("UnterExpVoll").unwrap_or(1.0),
                b.f32("ObereEXPVoll").unwrap_or(1.1),
            ],
            difficulty: [
                b.row_f32("ProzAW", BRONZE).unwrap_or(0.65),
                b.row_f32("ProzDmg", BRONZE).unwrap_or(0.9),
                b.row_f32("ProzRes", BRONZE).unwrap_or(1.0),
            ],
            regeneration: {
                let mut rates = default.regeneration;
                for (rate, name) in
                    rates
                        .iter_mut()
                        .zip(["RegInFight", "RegEnemyNear", "RegNoEnemy"])
                {
                    if let Some(value) = b.i32(name).and_then(|v| i16::try_from(v).ok()) {
                        *rate = f32::from(value);
                    }
                }
                rates
            },
            curves: Curves::of(b),
            // The cap and the start levels by the difficulty itself, bronze's first.
            level_cap: b
                .row_i32("LevelKap", 0)
                .and_then(|v| i16::try_from(v).ok())
                .map_or(default.level_cap, f32::from),
            start_levels: [1, 2].map(|i| {
                b.row_i32("StartLevMulti", i)
                    .and_then(|v| i16::try_from(v).ok())
                    .map_or(default.start_levels[i - 1], f32::from)
            }),
            skill_levels: {
                let mut levels = default.skill_levels;
                for (i, level) in levels.iter_mut().enumerate() {
                    if let Some(value) = b
                        .i32(&format!("skilllearn{}", i + 1))
                        .and_then(|v| u32::try_from(v).ok())
                    {
                        *level = value;
                    }
                }
                levels
            },
        }
    }

    /// The factor of a creature's level `level` (`0x5658F0`): between two of the points about
    /// it, the first's weight plus the level times the slope, the level taken whole rather than
    /// from the first point (the original's own); 1 outside the points.
    fn level_factor(&self, level: f32) -> f32 {
        self.points
            .windows(2)
            .find(|pair| pair[0].0 <= level && level <= pair[1].0)
            .map_or(1.0, |pair| {
                let ((s0, w0), (s1, w1)) = (pair[0], pair[1]);
                w0 + level * (w1 - w0) / (s1 - s0)
            })
    }
}

/// Whether a creature of level `level` takes on a hero of level `hero` (`0x542B20`, docs/re/fight.md,
/// "Who fights"): its level on bronze's scale, `E` (the game plays bronze), against the hero's,
/// `r = (E + 9)/(H + 9)` above the threshold `t` (`(H + 6)/(H + 9)` up to level 10, `(H − 6)/(H + 9)`
/// from 92, else `anteil`), by more than 0.04 of `voll − t`: when the hero would earn more than the
/// least experience from it.
pub fn takes_on(level: u32, hero: u32, (anteil, voll): (f32, f32)) -> bool {
    let (e, h) = (to_f32(level), to_f32(hero));
    let r = (e + 9.0) / (h + 9.0);
    let t = match hero {
        0..=10 => (h + 6.0) / (h + 9.0),
        92.. => (h - 6.0) / (h + 9.0),
        _ => anteil,
    };
    r > t && (r - t) / (voll - t) > 0.04
}

/// A level, or another whole number, as a float.
fn to_f32(level: u32) -> f32 {
    f32::from(u16::try_from(level).unwrap_or(u16::MAX))
}

/// The whole part of a float, 0 below 0.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a damage, whole and positive, far below the integers' range"
)]
fn whole(value: f32) -> u32 {
    value.max(0.0).floor() as u32
}

/// A creature type's values of `Creature.pak` that its fight takes: its six base attributes
/// (`+0x0C`), its six bonuses (`+0x2A`: a kind and a target; their values `+0x36` and the
/// opponents' classes `+0x3C`), the per cent of each kind's damage that it takes off on each
/// difficulty (`+0x42`), and its experience (`+0x08`).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Base {
    /// Its type, the record's id.
    pub id: u32,
    pub attributes: [u8; 6],
    pub bonus: [[u8; 2]; 6],
    pub bonus_value: [u8; 6],
    pub bonus_class: [u8; 6],
    pub damping: [[u8; 5]; KINDS],
    pub experience: [u16; 2],
    /// Its eight skill slots (`0x586C00`): `+0x14`, `+0x15` and, for a type not of class 1, the
    /// first six of `+0x16`; 0 for none.
    pub skills: [u8; 8],
}

impl Base {
    pub fn of(creature: &Creature) -> Self {
        Self {
            id: creature.id,
            attributes: creature.base,
            bonus: creature.bonus,
            bonus_value: creature.bonus_value,
            bonus_class: creature.bonus_class,
            damping: creature.damping,
            experience: creature.experience,
            skills: {
                let mut skills = [0; 8];
                skills[..2].copy_from_slice(&creature.skills);
                if creature.class != 1 {
                    skills[2..].copy_from_slice(&creature.skills_extra[..6]);
                }
                skills
            },
        }
    }
}

/// What a creature holds, as its values take it: a weapon (`0x546F80`) and a shield
/// (`0x54A660`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Holds {
    pub weapon: bool,
    pub shield: bool,
}

/// The skills a creature of `base` counts at level `level`, and their levels: an NPC the first
/// of its slots, none below level 3, one up to 6, two up to 20, three up to 35, else all eight,
/// each at a third of its level (`0x564D60`); a hero its two slots at level 1 (`0x586C00`;
/// **hypothesis**: the template's own levels are not read).
pub fn skills(base: &Base, level: u32, hero: bool) -> Vec<(u8, u32)> {
    let (count, at) = if hero {
        (2, 1)
    } else {
        let count = match level {
            0..3 => 0,
            3..=6 => 1,
            7..=20 => 2,
            21..=35 => 3,
            _ => 8,
        };
        (count, level / 3)
    };
    base.skills
        .iter()
        .take(count)
        .filter(|&&skill| skill != 0)
        .map(|&skill| (skill, at))
        .collect()
}

/// A skill's per cent at level `level` (`0x566070`), of the balance's triple `[off, S, w]`:
/// `off` at level 1, `w` at level `S + 1`, `off + 2(w − off)(1 − 1/(1 + (level − 1)/S))`; none
/// below level 1.
fn curve([off, steps, most]: [f32; 3], level: u32) -> f32 {
    if level < 1 || steps <= 0.0 {
        return 0.0;
    }
    off + 2.0 * (most - off) * (1.0 - 1.0 / (1.0 + to_f32(level - 1) / steps))
}

/// The balance's triples of the skills' curves (`0x566070`): the damages' (2, `WK…`), the
/// attack's and the defence's (8, `W…AW`, `W…VW`), the regeneration's (10, `HR…AW`), the
/// defence's again (9, `P…`, with a shield `P…S`), and each resistance's (11, `R…Ph`, `R…Fe`,
/// `R…Ma`, `R…Gi`).
#[derive(Clone, Debug, PartialEq)]
struct Curves {
    damage: [f32; 3],
    attack: [f32; 3],
    defence: [f32; 3],
    regeneration: [f32; 3],
    parry: [f32; 3],
    parry_shield: [f32; 3],
    resistances: [[f32; 3]; KINDS],
}

impl Default for Curves {
    /// The original's values.
    fn default() -> Self {
        Self {
            damage: [12.0, 50.0, 200.0],
            attack: [7.0, 50.0, 125.0],
            defence: [13.0, 50.0, 225.0],
            regeneration: [6.0, 50.0, 150.0],
            parry: [15.0, 50.0, 250.0],
            parry_shield: [25.0, 50.0, 375.0],
            resistances: [
                [12.0, 50.0, 250.0],
                [12.0, 50.0, 200.0],
                [12.0, 50.0, 200.0],
                [12.0, 50.0, 200.0],
            ],
        }
    }
}

impl Curves {
    /// The triples of `balance`, the original's where it lacks them.
    fn of(balance: &Balance) -> Self {
        let default = Self::default();
        let triple = |names: [&str; 3], or: [f32; 3]| {
            let mut values = or;
            for (value, name) in values.iter_mut().zip(names) {
                *value = balance.f32(name).unwrap_or(*value);
            }
            values
        };
        let resistance = |kind: usize, suffix: &str| {
            triple(
                [
                    &format!("Roff{suffix}"),
                    &format!("R__s{suffix}"),
                    &format!("R__w{suffix}"),
                ],
                default.resistances[kind],
            )
        };
        Self {
            damage: triple(["WKoff", "WK_s", "WK_w"], default.damage),
            attack: triple(["WoffAW", "W__sAW", "W__wAW"], default.attack),
            defence: triple(["WoffVW", "W__sVW", "W__wVW"], default.defence),
            regeneration: triple(["HRoffAW", "HR__sAW", "HR__wAW"], default.regeneration),
            parry: triple(["Poff", "P__s", "P__w"], default.parry),
            parry_shield: triple(["PoffS", "P__sS", "P__wS"], default.parry_shield),
            resistances: [
                resistance(0, "Ph"),
                resistance(1, "Fe"),
                resistance(2, "Ma"),
                resistance(3, "Gi"),
            ],
        }
    }
}

/// A creature's attack `A`, defence `D`, each kind's least and most damage and resistance, with
/// the balance's factors, the per cent of each kind's damage that it takes off, and its attack
/// speed in per cent.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Combat {
    pub attack: f32,
    pub defence: f32,
    pub damage: [[f32; 2]; KINDS],
    pub resistance: [f32; KINDS],
    pub damping: [u8; KINDS],
    pub speed: f32,
}

impl Combat {
    /// The speed of its attack's clip (`0x4E4CF0`): its attack speed over 100, times the
    /// factor of chained strikes (`+0x210`, 1 for a strike that is not chained).
    pub fn clip_speed(&self) -> f32 {
        self.speed / 100.0
    }
}

/// The attack speed of a creature of type `id` (`0x5796A0`, `0x584FE0`): 140 for every type but
/// the 5th's (the Wood Elf's), 100; no items, weapon skills or effects yet (#294).
pub fn attack_speed(id: u32) -> f32 {
    if id == 5 { 100.0 } else { 140.0 }
}

/// The attack, defence, damage and resistances of an NPC of `base` and level `level`, `armed`
/// when it holds a weapon (the recomputation `0x5796A0` and the attack record `0x568D70`;
/// docs/re/fight.md, "An NPC's attack, defence and damage"):
/// - the attributes `a_i = b_i + ⌊b_i(L − 1)/10⌋` and the level's factor
///   `F = (156 − BalStatOff)·L/156 + BalStatOff + 9`;
/// - `A = (a0 + a2)/2`, `D = 0.2·a0 + 0.8·a2`;
/// - unarmed, the bonuses of the kinds 1 to 7 (`x` per cent of an attribute or of the level) to a
///   kind's most (`x`) and least (`x/2 + 1`); armed or not, the bonuses to a kind's resistance;
///   a physical most of 0 takes `0.15(2a0 + a2) + L/2` to `0.175(a0 + a2) + L/2`;
/// - each kind's damage times `1 +` its part of the base attributes times `F` over `BalChar*D`;
/// - the resistances of the attributes over `BalanceRes*`, times `1 + F·b1/10` over `BalChar*R`.
///
/// - the skills `skills` (`0x566070`): 2 raises every damage multiplier, 8 the attack's and the
///   defence's, 9 the defence's (more with a shield), 11 each resistance's, by its curve's per
///   cent; an attack or a defence whose multiplier is not 1 is truncated.
///
/// No items, combat arts or bonuses of other kinds yet; an armed NPC's damage is the default's,
/// its weapon's unknown (#294; docs/decisions/2026-10-20-hits.md).
pub fn combat(
    base: &Base,
    level: u32,
    holds: Holds,
    skills: &[(u8, u32)],
    rules: &Rules,
) -> Combat {
    let armed = holds.weapon;
    let level = level.max(1);
    let b = base.attributes.map(f32::from);
    let a = base
        .attributes
        .map(|b| to_f32(u32::from(b) + u32::from(b) * (level - 1) / 10));
    let l = to_f32(level);
    let off = rules.stat_offset;
    let f = (156.0 - off) * l / 156.0 + off + 9.0;
    let mut damage = [[0.0_f32; 2]; KINDS];
    let mut resistance = [
        (a[3] + a[1]) / rules.resistances[0],
        a[2] / rules.resistances[1],
        a[4] / rules.resistances[2],
        (a[5] + a[1]) / rules.resistances[3],
    ];
    for i in 0..6 {
        let [kind, target] = base.bonus[i];
        // A bonus of a class holds only against an opponent of that class; none has one.
        if base.bonus_class[i] != 0 {
            continue;
        }
        let source = match kind {
            1 => a[0],
            2 => a[1],
            3 => a[3],
            4 => a[4],
            5 => a[2],
            6 => a[5],
            7 => l,
            _ => continue,
        };
        let x = f32::from(base.bonus_value[i]) * source / 100.0;
        match usize::from(target) {
            t @ 1..=4 if !armed => {
                damage[t - 1][1] += x;
                damage[t - 1][0] += x / 2.0 + 1.0;
            }
            t @ 5..=8 => resistance[t - 5] += x,
            _ => {}
        }
    }
    if damage[0][1] == 0.0 {
        damage[0][1] += 0.15 * (2.0 * a[0] + a[2]) + l / 2.0;
        damage[0][0] += 0.175 * (a[0] + a[2]) + l / 2.0;
    }
    // The skills' parts of the multipliers.
    let curves = &rules.curves;
    let (mut damage_skill, mut attack_skill, mut defence_skill) = (0.0, 1.0, 1.0);
    let mut resistance_skill = [0.0; KINDS];
    for &(skill, at) in skills {
        match skill {
            2 => damage_skill += curve(curves.damage, at) / 100.0,
            8 => {
                attack_skill += curve(curves.attack, at) / 100.0;
                defence_skill += curve(curves.defence, at) / 100.0;
            }
            9 => {
                let parry = if holds.shield {
                    curves.parry_shield
                } else {
                    curves.parry
                };
                defence_skill += curve(parry, at) / 100.0;
            }
            11 => {
                for (value, triple) in resistance_skill.iter_mut().zip(curves.resistances) {
                    *value += curve(triple, at) / 100.0;
                }
            }
            _ => {}
        }
    }
    let damage_parts = [b[0], b[0] + b[2], b[4], b[1] + b[5]];
    let [dmg, res] = rules.balance;
    for (kind, ends) in damage.iter_mut().enumerate() {
        let factor =
            1.0 + damage_skill + f * damage_parts[kind] / 10.0 / rules.damage_divisors[kind];
        *ends = ends.map(|end| end.max(0.0) * factor.max(0.0) * dmg);
    }
    for (kind, value) in resistance.iter_mut().enumerate() {
        let factor =
            1.0 + resistance_skill[kind] + f * b[1] / 10.0 / rules.resistance_divisors[kind];
        *value = value.max(0.0) * factor.max(0.0) * res;
    }
    let multiplied = |value: f32, factor: f32| {
        if factor == 1.0 {
            value
        } else {
            to_f32(whole(value * factor))
        }
    };
    Combat {
        attack: multiplied((a[0] * ATTACK[0] + a[2] * ATTACK[1]) / 100.0, attack_skill),
        defence: multiplied(
            (a[0] * DEFENCE[0] + a[2] * DEFENCE[1]) / 100.0,
            defence_skill,
        ),
        damage,
        resistance,
        damping: base.damping.map(|by_difficulty| by_difficulty[0]),
        speed: attack_speed(base.id),
    }
}

/// The values of a strike between `attacker` and `defender` on bronze (`0x56B130`): a creature
/// striking the hero has its attack times `ProzAW` and its damages times `ProzDmg`, and the
/// exponent of the damage `k = −log₂(1 − p)`, `p` half of 1 plus a hundredth of each level it has
/// above the hero's; the hero striking a creature meets its defence times `ProzAW` and its
/// resistances times `ProzRes`; between creatures, as they are, `k` 1. `hero` tells which side
/// is the hero's: the attacker's, the defender's or neither.
pub fn weighed(
    mut attacker: Combat,
    mut defender: Combat,
    hero: Option<Side>,
    levels: (u32, u32),
    rules: &Rules,
) -> (Combat, Combat, f32) {
    let [aw, dmg, res] = rules.difficulty;
    let mut k = 1.0;
    match hero {
        Some(Side::Defender) => {
            attacker.attack *= aw;
            for ends in &mut attacker.damage {
                *ends = ends.map(|end| end * dmg);
            }
            let above = to_f32(levels.0.saturating_sub(levels.1));
            let p = (0.5 * (1.0 + above * 0.01)).min(0.99);
            k = -(1.0 - p).log2();
        }
        Some(Side::Attacker) => {
            defender.defence *= aw;
            for value in &mut defender.resistance {
                *value *= res;
            }
        }
        None => {}
    }
    (attacker, defender, k)
}

/// A side of a strike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Attacker,
    Defender,
}

/// Each kind's damage of a strike (`0x568D70`): a whole number of steps between the kind's least
/// and most, the steps hundredths while the physical most is below 100, tenths below 1,000, else
/// units; 15 physical when every kind's is below 1.
fn roll_damage(damage: &[[f32; 2]; KINDS], rng: &mut Rng) -> [f32; KINDS] {
    let step = match damage[0][1] {
        most if most < 100.0 => 100.0,
        most if most < 1000.0 => 10.0,
        _ => 1.0,
    };
    let mut rolled = damage.map(|[least, most]| {
        let (least, most) = (whole(least * step), whole(most * step));
        let (low, high) = (least.min(most), least.max(most));
        to_f32(low + rng.below(high - low + 1)) / step
    });
    if rolled.iter().all(|&d| d < 1.0) {
        rolled[0] = 15.0;
    }
    rolled
}

/// The damage that a strike of `attacker` deals `defender`, its share `share` of a strike, or
/// none when it misses (`0x56B130`): the chance `A/(A + D)` (`0x43EEE0`) against a roll of 0 to
/// 1,000 over 1,000; each kind's damage `d` rolled, then `d·(1 − 1/(1 + d/r)^k)` against the
/// kind's resistance `r`, `d²/(d + r)` for the exponent `k` of 1, less the defender's damping
/// of bronze; the sum times the share, whole.
pub fn strike(
    attacker: &Combat,
    defender: &Combat,
    (share, k): (f32, f32),
    rng: &mut Rng,
) -> Option<u32> {
    let chance = if defender.defence > 0.0 {
        attacker.attack / (attacker.attack + defender.defence)
    } else {
        1.0
    };
    if chance < to_f32(rng.below(1001)) / 1000.0 {
        return None;
    }
    let total: f32 = roll_damage(&attacker.damage, rng)
        .iter()
        .zip(defender.resistance)
        .zip(defender.damping)
        .map(|((&d, r), damping)| {
            let dealt = match (d > 0.0, r > 0.0) {
                (false, _) => 0.0,
                (true, false) => d,
                (true, true) => d * (1.0 - 1.0 / (1.0 + d / r).powf(k)),
            };
            dealt * f32::from(100_u8.saturating_sub(damping)) / 100.0
        })
        .sum();
    Some(whole(total * share))
}

/// A creature's level (docs/re/spawning.md): the hero's, `hero`, held within its sector's lowest
/// and highest, and for one of a fill, `jitter`, moved by 0 to 1 below 6, −1 to 2 below 30, −2 to
/// 3 below 90, −3 to 6 below 250; at least 1.
pub fn level(values: &SpawnValues, hero: u32, jitter: Option<&mut Rng>) -> u32 {
    let level = hero.clamp(values.lowest.min(values.highest), values.highest);
    let Some(rng) = jitter else {
        return level.max(1);
    };
    let (down, up) = match level {
        0..6 => (0, 1),
        6..30 => (1, 2),
        30..90 => (2, 3),
        _ => (3, 6),
    };
    let moved = level + rng.below(down + up + 1);
    moved.saturating_sub(down).max(1)
}

/// The most health of a creature of `Creature.pak`'s base bytes `base` and level `level`
/// (`0x5658F0`): with `a` and `b` the bytes 0 and 3, `S = a + b + ⌊(L − 1)a/10⌋ + ⌊(L − 1)b/10⌋`
/// and `g = 1.5 + 0.004 · max(0, 60 − a − b)`, `⌊(b0 + b2) · S^(g − 1) · 10/31⌋`, 100 for 0,
/// times the level's factor of a creature not a hero and the difficulty's (`rules`). No items,
/// skills, bosses or champions yet (docs/decisions/2026-10-20-health.md).
pub fn most_health(base: [u8; 6], level: u32, rules: &Rules) -> u32 {
    let level_f32 = f32::from(u16::try_from(level).unwrap_or(u16::MAX));
    health_of(
        base,
        level,
        f64::from(rules.level_factor(level_f32) * rules.factor),
    )
}

/// The most health of a hero of `Creature.pak`'s base bytes `base` and level `level`
/// (`0x5658F0`): as a creature's, without the factors of a creature not a hero.
pub fn hero_health(base: [u8; 6], level: u32) -> u32 {
    health_of(base, level, 1.0)
}

/// The most health of `base` at `level`, times `factor` (`0x5658F0`).
fn health_of(base: [u8; 6], level: u32, factor: f64) -> u32 {
    let (a, b) = (f64::from(base[0]), f64::from(base[3]));
    let l = f64::from(level.max(1) - 1);
    let s = a + b + (l * a / 10.0).floor() + (l * b / 10.0).floor();
    let g = 1.5 + 0.004 * (60.0 - a - b).max(0.0);
    let most = ((a + f64::from(base[2])) * s.powf(g - 1.0) * 10.0 / 31.0).floor();
    let most = if most <= 0.0 { 100.0 } else { most };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a health, whole and positive, far below the integers' range"
    )]
    let most = (most * factor).floor().max(1.0) as u32;
    most
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_most_health_grows_with_the_level_by_the_originals_formula() {
        let rules = Rules::default();
        // b0 20, b2 10, b3 15: S = 35, g = 1.5 + 0.004 · 25 = 1.6; ⌊30 · 35^0.6 · 10/31⌋.
        let base = [20, 0, 10, 15, 0, 0];
        let expected = (30.0_f64 * 35.0_f64.powf(0.6) * 10.0 / 31.0).floor();
        assert_eq!(f64::from(most_health(base, 1, &rules)), expected);
        assert!(most_health(base, 10, &rules) > most_health(base, 1, &rules));
        // A base that gives nothing gives 100.
        assert_eq!(most_health([0; 6], 1, &rules), 100);
    }

    #[test]
    fn the_level_factor_takes_the_level_whole_between_its_points() {
        let rules = Rules {
            points: vec![
                (1.0, 0.75),
                (4.0, 0.8),
                (10.0, 0.9),
                (20.0, 1.0),
                (35.0, 1.1),
            ],
            ..Rules::default()
        };
        // Between 1 and 4: 0.75 + L · 0.05/3, the level whole.
        assert!((rules.level_factor(1.0) - (0.75 + 0.05 / 3.0)).abs() < 1e-6);
        assert!((rules.level_factor(20.0) - (0.9 + 20.0 * 0.1 / 10.0)).abs() < 1e-6);
        assert!((rules.level_factor(40.0) - 1.0).abs() < 1e-6);
    }

    /// A wolf (512) and a goblin warrior (76) of `Creature.pak`, as far as their fight goes.
    fn a_wolf() -> Base {
        Base {
            id: 512,
            attributes: [8, 10, 18, 13, 0, 1],
            bonus: [[1, 1], [2, 5], [0, 0], [0, 0], [0, 0], [0, 0]],
            bonus_value: [50, 30, 0, 0, 0, 0],
            skills: [2, 11, 0, 0, 0, 0, 0, 0],
            ..Base::default()
        }
    }

    fn a_goblin() -> Base {
        Base {
            id: 76,
            attributes: [15, 20, 22, 12, 0, 4],
            bonus: [[1, 1], [2, 5], [2, 6], [0, 0], [0, 0], [0, 0]],
            bonus_value: [120, 20, 20, 0, 0, 0],
            ..Base::default()
        }
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn an_npcs_attack_defence_damage_and_resistance_follow_the_originals_examples() {
        let rules = Rules::default();
        // docs/re/fight.md, "An NPC's attack, defence and damage": level 1, unarmed.
        let wolf = combat(&a_wolf(), 1, Holds::default(), &[], &rules);
        assert!(near(wolf.attack, 13.0) && near(wolf.defence, 16.0));
        assert!(near(wolf.damage[0][0], 3.33) && near(wolf.damage[0][1], 4.43));
        assert_eq!(wolf.damage[1..], [[0.0; 2]; 3]);
        assert!(near(wolf.resistance[0], 4.48));
        let goblin = combat(&a_goblin(), 1, Holds::default(), &[], &rules);
        assert!(near(goblin.attack, 18.5) && near(goblin.defence, 20.6));
        assert!(near(goblin.damage[0][0], 12.04) && near(goblin.damage[0][1], 21.67));
        assert!(near(goblin.resistance[0], 6.62));
        // Armed, the damage bonus does not hold: the default's, `0.15(2a0 + a2) + L/2` to
        // `0.175(a0 + a2) + L/2`, times the same multiplier.
        let armed = combat(
            &a_goblin(),
            1,
            Holds {
                weapon: true,
                shield: false,
            },
            &[],
            &rules,
        );
        let factor = goblin.damage[0][1] / 18.0;
        assert!(near(armed.damage[0][1], (0.15 * 52.0 + 0.5) * factor));
        assert!(near(armed.damage[0][0], (0.175 * 37.0 + 0.5) * factor));
        assert!(near(armed.resistance[0], goblin.resistance[0]));
        // The level raises the attributes by a tenth each: a level-10 wolf's are 15, 19, 34, 24.
        let older = combat(&a_wolf(), 10, Holds::default(), &[], &rules);
        assert!(near(older.attack, 24.5) && near(older.defence, 30.2));
    }

    #[test]
    fn a_strike_hits_by_the_chance_of_attack_against_defence() {
        let rules = Rules::default();
        let (wolf, goblin) = (
            combat(&a_wolf(), 1, Holds::default(), &[], &rules),
            combat(&a_goblin(), 1, Holds::default(), &[], &rules),
        );
        let mut rng = Rng::of(50, 39, 0);
        let hits = (0..10_000)
            .filter(|_| strike(&goblin, &wolf, (1.0, 1.0), &mut rng).is_some())
            .count();
        // A/(A + D) = 18.5/34.5 = 0.536.
        assert!((5_100..5_650).contains(&hits), "{hits}");
        // Against no defence every strike hits.
        let open = Combat {
            defence: 0.0,
            ..wolf
        };
        assert!((0..100).all(|_| strike(&wolf, &open, (1.0, 1.0), &mut rng).is_some()));
    }

    #[test]
    fn a_hits_damage_is_less_by_the_resistance_and_the_damping() {
        let mut rng = Rng::of(50, 39, 0);
        let attacker = Combat {
            attack: 10.0,
            defence: 0.0,
            damage: [[10.0, 10.0], [0.0; 2], [0.0; 2], [0.0; 2]],
            resistance: [0.0; 4],
            damping: [0; 4],
            speed: 140.0,
        };
        let defender = Combat {
            resistance: [10.0, 0.0, 0.0, 0.0],
            ..attacker
        };
        // 10²/(10 + 10) = 5; with a damping of 40 per cent, 3; half a strike, 2.
        assert_eq!(strike(&attacker, &defender, (1.0, 1.0), &mut rng), Some(5));
        let damped = Combat {
            damping: [40, 0, 0, 0],
            ..defender
        };
        assert_eq!(strike(&attacker, &damped, (1.0, 1.0), &mut rng), Some(3));
        assert_eq!(strike(&attacker, &defender, (0.5, 1.0), &mut rng), Some(2));
        // Every kind below 1: 15 physical.
        let weak = Combat {
            damage: [[0.5, 0.9], [0.0; 2], [0.0; 2], [0.0; 2]],
            ..attacker
        };
        assert_eq!(strike(&weak, &attacker, (1.0, 1.0), &mut rng), Some(15));
    }

    #[test]
    fn a_damage_is_rolled_between_its_ends_in_steps_by_its_size() {
        let mut rng = Rng::of(50, 39, 0);
        let damage = [[3.33, 4.43], [0.0; 2], [0.0; 2], [0.0; 2]];
        let rolls: Vec<f32> = (0..1000)
            .map(|_| roll_damage(&damage, &mut rng)[0])
            .collect();
        assert!(rolls.iter().all(|&d| (3.33..=4.43).contains(&d)));
        // Hundredths below 100.
        assert!(rolls.iter().any(|&d| (d * 10.0).fract() > 0.01));
        // Tenths from 100.
        let large = [[150.0, 160.0], [0.0; 2], [0.0; 2], [0.0; 2]];
        assert!((0..100).all(|_| {
            let d = roll_damage(&large, &mut rng)[0] * 10.0;
            (d - d.round()).abs() < 1e-3
        }));
    }

    #[test]
    fn an_npc_counts_its_first_skills_by_its_level_at_a_third_of_it() {
        let wolf = a_wolf();
        assert!(skills(&wolf, 1, false).is_empty());
        assert_eq!(skills(&wolf, 4, false), [(2, 1)]);
        assert_eq!(skills(&wolf, 10, false), [(2, 3), (11, 3)]);
        // A hero its two slots at level 1.
        assert_eq!(skills(&wolf, 10, true), [(2, 1), (11, 1)]);
    }

    #[test]
    fn a_skills_curve_runs_from_its_offset_to_its_value_at_s_plus_one() {
        let triple = [12.0, 50.0, 200.0];
        assert!((curve(triple, 1) - 12.0).abs() < 1e-4);
        assert!((curve(triple, 51) - 200.0).abs() < 1e-3);
        assert!((curve(triple, 3) - (12.0 + 376.0 * (1.0 - 1.0 / 1.04))).abs() < 1e-3);
        assert!(curve(triple, 0).abs() < f32::EPSILON);
    }

    #[test]
    fn the_skills_raise_a_level_ten_wolfs_damage_and_resistance_as_the_originals() {
        let rules = Rules::default();
        let wolf = a_wolf();
        // docs/re/fight.md: the skills 2 and 11 at level 3.
        let older = combat(
            &wolf,
            10,
            Holds::default(),
            &skills(&wolf, 10, false),
            &rules,
        );
        assert!(near(older.damage[0][0], 6.66) && near(older.damage[0][1], 10.51));
        assert!((older.resistance[0] - 11.0).abs() < 0.02);
        assert!(near(older.attack, 24.5) && near(older.defence, 30.2));
        // The skill 8 raises the attack and the defence, truncated: 24.5 · 1.07 and 30.2 · 1.13.
        let trained = combat(&wolf, 10, Holds::default(), &[(8, 1)], &rules);
        assert!(near(trained.attack, 26.0) && near(trained.defence, 34.0));
        // The skill 10 the regeneration: 44 · 1.06.
        let base = Base {
            attributes: [0, 0, 0, 22, 0, 0],
            ..Base::default()
        };
        assert!((regeneration(&base, 1, &[(10, 1)], &rules) - 46.64).abs() < 1e-3);
    }

    #[test]
    fn the_hero_and_the_creatures_strike_each_other_by_bronzes_factors() {
        let rules = Rules::default();
        let (wolf, goblin) = (
            combat(&a_wolf(), 1, Holds::default(), &[], &rules),
            combat(&a_goblin(), 1, Holds::default(), &[], &rules),
        );
        // A creature striking the hero: its attack times 0.65, its damage times 0.9.
        let (a, d, k) = weighed(wolf, goblin, Some(Side::Defender), (1, 1), &rules);
        assert!(near(a.attack, 13.0 * 0.65) && near(a.damage[0][1], 4.43 * 0.9));
        assert_eq!(d, goblin);
        assert!((k - 1.0).abs() < 1e-6);
        // Ten levels above the hero's: `p` 0.55, `k = −log₂ 0.45`.
        let (.., k) = weighed(wolf, goblin, Some(Side::Defender), (11, 1), &rules);
        assert!((k - -(0.45_f32).log2()).abs() < 1e-5);
        // The hero striking a creature: its defence times 0.65.
        let (a, d, k) = weighed(goblin, wolf, Some(Side::Attacker), (1, 11), &rules);
        assert_eq!(a, goblin);
        assert!(near(d.defence, 16.0 * 0.65) && near(d.resistance[0], 4.48));
        assert!((k - 1.0).abs() < 1e-6);
        // Between creatures, as they are.
        assert_eq!(
            weighed(wolf, goblin, None, (9, 1), &rules),
            (wolf, goblin, 1.0)
        );
    }

    #[test]
    fn a_larger_exponent_takes_more_of_the_damage_through_the_resistance() {
        let mut rng = Rng::of(50, 39, 0);
        let attacker = Combat {
            attack: 10.0,
            defence: 0.0,
            damage: [[10.0, 10.0], [0.0; 2], [0.0; 2], [0.0; 2]],
            resistance: [0.0; 4],
            damping: [0; 4],
            speed: 140.0,
        };
        let defender = Combat {
            resistance: [10.0, 0.0, 0.0, 0.0],
            ..attacker
        };
        // `10·(1 − 1/2²)` = 7.5 for `k` 2.
        assert_eq!(strike(&attacker, &defender, (1.0, 2.0), &mut rng), Some(7));
    }

    #[test]
    fn the_heros_health_has_no_factor_of_a_creature() {
        let rules = Rules {
            factor: 2.0,
            ..Rules::default()
        };
        let base = [22, 19, 25, 22, 22, 17];
        assert_eq!(most_health(base, 1, &rules), 2 * hero_health(base, 1));
        assert_eq!(hero_health(base, 1), health_of(base, 1, 1.0));
    }

    #[test]
    fn regeneration_gives_one_health_for_each_4096_gathered() {
        // `2·a3`: a base of 22 at level 1, 44; at level 11, 2·(22 + 22) = 88.
        let base = Base {
            attributes: [0, 0, 0, 22, 0, 0],
            ..Base::default()
        };
        assert!((regeneration(&base, 1, &[], &Rules::default()) - 44.0).abs() < 1e-6);
        assert!((regeneration(&base, 11, &[], &Rules::default()) - 88.0).abs() < 1e-6);
        let mut regenerates = Regenerates::new(44.0);
        // 44 · 128 a second: 5,632 after one, a health and 5,632 − 4,096 = 1,536 kept as its
        // low ten bits, 512.
        assert_eq!(regenerates.gather(128.0, 0.5), 0);
        assert_eq!(regenerates.gather(128.0, 0.5), 1);
        assert!((regenerates.kept - 512.0).abs() < 1e-3);
    }

    #[test]
    fn a_kill_earns_the_creatures_experience_by_the_levels() {
        let rules = Rules::default();
        let worth = [20, 5];
        // Of the hero's level, or one above, the whole `a + E·b`.
        assert_eq!(experience_of(worth, 1, 1, &rules), 25);
        assert_eq!(experience_of(worth, 2, 1, &rules), 30);
        // Above `ObereEXPVoll`, less by `ObereEXPVoll/r`: 35 · 1.1/1.2.
        assert_eq!(experience_of(worth, 3, 1, &rules), 32);
        // Below the hero's: at the threshold 0.04, above it `(r − t)/(1 − t)`.
        assert_eq!(experience_of(worth, 2, 5, &rules), 1);
        assert_eq!(experience_of(worth, 3, 5, &rules), 11);
        // Bronze's `ProzExp` and the balance's values.
        let more = Rules {
            experience: [2.0, 0.85, 1.0, 1.1],
            ..Rules::default()
        };
        assert_eq!(experience_of(worth, 1, 1, &more), 50);
    }

    #[test]
    fn every_creature_strikes_at_140_per_cent_but_the_wood_elf() {
        assert!((attack_speed(76) - 140.0).abs() < f32::EPSILON);
        assert!((attack_speed(5) - 100.0).abs() < f32::EPSILON);
        let goblin = combat(&a_goblin(), 1, Holds::default(), &[], &Rules::default());
        assert!((goblin.clip_speed() - 1.4).abs() < 1e-6);
    }

    #[test]
    fn the_levels_end_at_the_originals_experience() {
        assert_eq!(
            [1, 2, 3, 4, 5, 9, 10].map(level_end),
            [300, 1_200, 3_000, 5_900, 10_200, 49_500, 66_900]
        );
        // The top level's end, one above the original's cap of the experience.
        assert_eq!(level_end(TOP_LEVEL), 2_586_931_600);
        assert_eq!(level_of(299, 1), 1);
        assert_eq!(level_of(300, 1), 2);
        assert_eq!(level_of(1_199, 1), 2);
        assert_eq!(level_of(1_200, 2), 3);
        assert_eq!(level_of(49_500, 3), 10);
        assert_eq!(level_of(u64::MAX, 1), TOP_LEVEL);
    }

    #[test]
    fn a_level_gives_an_attribute_point_and_skill_points_by_the_slots() {
        let rules = Rules::default();
        let mut experience = Experience::default();
        assert!(!experience.add(299, &rules));
        // Level 2: one attribute point, one skill point.
        assert!(experience.add(1, &rules));
        assert_eq!(
            (
                experience.level,
                experience.attribute_points,
                experience.skill_points
            ),
            (2, 1, 1)
        );
        // Level 3: another attribute point, two skill points, a third skill to learn.
        assert!(experience.add(900, &rules));
        assert_eq!(
            (
                experience.level,
                experience.attribute_points,
                experience.skill_points
            ),
            (3, 2, 3)
        );
        assert_eq!(experience.to_learn, 1);
    }

    #[test]
    fn a_high_heros_kills_earn_less_by_the_cap_the_decay_and_the_fade() {
        let rules = Rules::default();
        let worth = [52, 48];
        // Up to level 50 the whole; at 60 by (50/60)².
        assert_eq!(experience_of(worth, 1, 1, &rules), 100);
        let at_60 = experience_of(worth, 60, 60, &rules);
        assert!(
            (to_f32(at_60) - 2_932.0 * 2_500.0 / 3_600.0).abs() <= 1.0,
            "{at_60}"
        );
        // At 80 also by the fade of bronze, (101 − 80)/31.
        let at_80 = experience_of(worth, 80, 80, &rules);
        let expected = 3_892.0 * 2_500.0 / 6_400.0 * 21.0 / 31.0;
        assert!((to_f32(at_80) - expected).abs() <= 1.0, "{at_80}");
        // From 101 on bronze, 1 a kill.
        assert_eq!(experience_of(worth, 101, 101, &rules), 1);
        assert!((decay(110) - 0.985_f32.powi(11)).abs() < 1e-5);
        assert!((decay(99) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_creature_takes_on_a_hero_from_whom_it_is_worth_experience() {
        let balance = (0.85, 1.0);
        // A hero of level 1 is taken on by every creature; of level 5, by those of 3 and up; of
        // level 20, by those of 16 and up.
        assert!((1..50).all(|level| takes_on(level, 1, balance)));
        assert!(!takes_on(2, 5, balance) && takes_on(3, 5, balance));
        assert!(!takes_on(15, 20, balance) && takes_on(16, 20, balance));
    }

    #[test]
    fn a_creature_takes_the_heros_level_within_its_sectors() {
        let start = SpawnValues {
            density: 50,
            lowest: 1,
            highest: 4,
        };
        let west = SpawnValues {
            density: 50,
            lowest: 25,
            highest: 38,
        };
        assert_eq!(level(&start, HERO_LEVEL, None), 1);
        assert_eq!(level(&west, HERO_LEVEL, None), 25);
        let mut rng = Rng::of(50, 39, 0);
        assert!((0..200).all(|_| (1..=2).contains(&level(&start, HERO_LEVEL, Some(&mut rng)))));
        assert!((0..200).all(|_| (24..=27).contains(&level(&west, HERO_LEVEL, Some(&mut rng)))));
    }
}
