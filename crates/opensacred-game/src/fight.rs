//! The fight's rules as far as the game follows them (docs/re/fight.md): the creatures' levels,
//! their health, their attack, defence, damage and resistances, and a strike's hit.

use bevy::prelude::*;
use opensacred_data::tables::{Balance, Creature};

use crate::game::{AppState, FONT};
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

/// Resolves each hit (`0x56B130`): a target that has no health yet (the hero, #292), or that is
/// dying, ignores it; else the strike may miss, and its damage takes the target's health down,
/// shows over it, and at 0 the target dies (the AI's state 9, `0x53E7C0`), one of its two dying
/// clips at random.
fn resolve(
    mut commands: Commands,
    mut hits: MessageReader<Hit>,
    mut dice: ResMut<Dice>,
    attackers: Query<&Combat>,
    mut targets: Query<(&mut Health, &Combat, &GlobalTransform), Without<Dies>>,
) {
    for hit in hits.read() {
        let (Ok(attacker), Ok((mut health, defender, at))) =
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
        let Some(damage) = strike(attacker, defender, hit.share, &mut dice.0) else {
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
                label: None,
            },
            DespawnOnExit(AppState::InGame),
        ));
        if health.current == 0 {
            commands
                .entity(hit.target)
                .insert(Dies::new(dice.0.below(2)));
        }
    }
}

/// How long a damage's number shows, in seconds, how high over the target's feet it starts and
/// how far it rises meanwhile, in world units, and its colour (**hypothesis**: the original's
/// number is not read).
const NUMBER_SHOWS: f32 = 1.0;
const NUMBER_ABOVE: f32 = 110.0;
const NUMBER_RISES: f32 = 60.0;
const NUMBER_COLOR: Color = Color::srgb(1.0, 0.85, 0.3);
const NUMBER_SIZE: f32 = 16.0;
/// How wide a number's box is, in logical pixels, centred over the place.
const NUMBER_WIDTH: f32 = 80.0;

/// A damage's number over the place of its target, rising until its time is out.
#[derive(Component)]
struct Number {
    at: Vec3,
    left: f32,
    text: String,
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
                        TextColor(NUMBER_COLOR),
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
            .add_systems(Update, (resolve, rise).chain());
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
/// (`BalanceRes*`), and the factors of all damages and resistances (`BalanceDmg`, `BalanceRes`).
#[derive(Clone, Debug, PartialEq)]
pub struct Rules {
    points: Vec<(f32, f32)>,
    factor: f32,
    stat_offset: f32,
    damage_divisors: [f32; KINDS],
    resistance_divisors: [f32; KINDS],
    resistances: [f32; KINDS],
    balance: [f32; 2],
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
/// opponents' classes `+0x3C`) and the per cent of each kind's damage that it takes off on each
/// difficulty (`+0x42`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Base {
    pub attributes: [u8; 6],
    pub bonus: [[u8; 2]; 6],
    pub bonus_value: [u8; 6],
    pub bonus_class: [u8; 6],
    pub damping: [[u8; 5]; KINDS],
}

impl Base {
    pub fn of(creature: &Creature) -> Self {
        Self {
            attributes: creature.base,
            bonus: creature.bonus,
            bonus_value: creature.bonus_value,
            bonus_class: creature.bonus_class,
            damping: creature.damping,
        }
    }
}

/// A creature's attack `A`, defence `D`, each kind's least and most damage and resistance, with
/// the balance's factors, and the per cent of each kind's damage that it takes off.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Combat {
    pub attack: f32,
    pub defence: f32,
    pub damage: [[f32; 2]; KINDS],
    pub resistance: [f32; KINDS],
    pub damping: [u8; KINDS],
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
/// No skills (#293), items or bonuses of other kinds yet; an armed NPC's damage is the
/// default's, its weapon's unknown (#294; docs/decisions/2026-10-20-hits.md).
pub fn combat(base: &Base, level: u32, armed: bool, rules: &Rules) -> Combat {
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
    let damage_parts = [b[0], b[0] + b[2], b[4], b[1] + b[5]];
    let [dmg, res] = rules.balance;
    for (kind, ends) in damage.iter_mut().enumerate() {
        let factor = 1.0 + f * damage_parts[kind] / 10.0 / rules.damage_divisors[kind];
        *ends = ends.map(|end| end.max(0.0) * factor.max(0.0) * dmg);
    }
    for (kind, value) in resistance.iter_mut().enumerate() {
        let factor = 1.0 + f * b[1] / 10.0 / rules.resistance_divisors[kind];
        *value = value.max(0.0) * factor.max(0.0) * res;
    }
    Combat {
        attack: (a[0] * ATTACK[0] + a[2] * ATTACK[1]) / 100.0,
        defence: (a[0] * DEFENCE[0] + a[2] * DEFENCE[1]) / 100.0,
        damage,
        resistance,
        damping: base.damping.map(|by_difficulty| by_difficulty[0]),
    }
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
/// 1,000 over 1,000; each kind's damage rolled, then `d²/(d + r)` against the kind's
/// resistance `r`, less the defender's damping of bronze; the sum times the share, whole.
pub fn strike(attacker: &Combat, defender: &Combat, share: f32, rng: &mut Rng) -> Option<u32> {
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
            let dealt = if d + r > 0.0 { d * d / (d + r) } else { 0.0 };
            dealt * f32::from(100_u8.saturating_sub(damping)) / 100.0
        })
        .sum();
    Some(whole(total * share))
}

/// A creature's level (docs/re/spawning.md): the hero's held within its sector's lowest and
/// highest, and for one of a fill, `jitter`, moved by 0 to 1 below 6, −1 to 2 below 30, −2 to 3
/// below 90, −3 to 6 below 250; at least 1.
pub fn level(values: &SpawnValues, jitter: Option<&mut Rng>) -> u32 {
    let level = HERO_LEVEL.clamp(values.lowest.min(values.highest), values.highest);
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
    let (a, b) = (f64::from(base[0]), f64::from(base[3]));
    let l = f64::from(level.max(1) - 1);
    let s = a + b + (l * a / 10.0).floor() + (l * b / 10.0).floor();
    let g = 1.5 + 0.004 * (60.0 - a - b).max(0.0);
    let most = ((a + f64::from(base[2])) * s.powf(g - 1.0) * 10.0 / 31.0).floor();
    let most = if most <= 0.0 { 100.0 } else { most };
    let level = f32::from(u16::try_from(level).unwrap_or(u16::MAX));
    let factor = f64::from(rules.level_factor(level) * rules.factor);
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
            attributes: [8, 10, 18, 13, 0, 1],
            bonus: [[1, 1], [2, 5], [0, 0], [0, 0], [0, 0], [0, 0]],
            bonus_value: [50, 30, 0, 0, 0, 0],
            ..Base::default()
        }
    }

    fn a_goblin() -> Base {
        Base {
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
        let wolf = combat(&a_wolf(), 1, false, &rules);
        assert!(near(wolf.attack, 13.0) && near(wolf.defence, 16.0));
        assert!(near(wolf.damage[0][0], 3.33) && near(wolf.damage[0][1], 4.43));
        assert_eq!(wolf.damage[1..], [[0.0; 2]; 3]);
        assert!(near(wolf.resistance[0], 4.48));
        let goblin = combat(&a_goblin(), 1, false, &rules);
        assert!(near(goblin.attack, 18.5) && near(goblin.defence, 20.6));
        assert!(near(goblin.damage[0][0], 12.04) && near(goblin.damage[0][1], 21.67));
        assert!(near(goblin.resistance[0], 6.62));
        // Armed, the damage bonus does not hold: the default's, `0.15(2a0 + a2) + L/2` to
        // `0.175(a0 + a2) + L/2`, times the same multiplier.
        let armed = combat(&a_goblin(), 1, true, &rules);
        let factor = goblin.damage[0][1] / 18.0;
        assert!(near(armed.damage[0][1], (0.15 * 52.0 + 0.5) * factor));
        assert!(near(armed.damage[0][0], (0.175 * 37.0 + 0.5) * factor));
        assert!(near(armed.resistance[0], goblin.resistance[0]));
        // The level raises the attributes by a tenth each: a level-10 wolf's are 15, 19, 34, 24.
        let older = combat(&a_wolf(), 10, false, &rules);
        assert!(near(older.attack, 24.5) && near(older.defence, 30.2));
    }

    #[test]
    fn a_strike_hits_by_the_chance_of_attack_against_defence() {
        let rules = Rules::default();
        let (wolf, goblin) = (
            combat(&a_wolf(), 1, false, &rules),
            combat(&a_goblin(), 1, false, &rules),
        );
        let mut rng = Rng::of(50, 39, 0);
        let hits = (0..10_000)
            .filter(|_| strike(&goblin, &wolf, 1.0, &mut rng).is_some())
            .count();
        // A/(A + D) = 18.5/34.5 = 0.536.
        assert!((5_100..5_650).contains(&hits), "{hits}");
        // Against no defence every strike hits.
        let open = Combat {
            defence: 0.0,
            ..wolf
        };
        assert!((0..100).all(|_| strike(&wolf, &open, 1.0, &mut rng).is_some()));
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
        };
        let defender = Combat {
            resistance: [10.0, 0.0, 0.0, 0.0],
            ..attacker
        };
        // 10²/(10 + 10) = 5; with a damping of 40 per cent, 3; half a strike, 2.
        assert_eq!(strike(&attacker, &defender, 1.0, &mut rng), Some(5));
        let damped = Combat {
            damping: [40, 0, 0, 0],
            ..defender
        };
        assert_eq!(strike(&attacker, &damped, 1.0, &mut rng), Some(3));
        assert_eq!(strike(&attacker, &defender, 0.5, &mut rng), Some(2));
        // Every kind below 1: 15 physical.
        let weak = Combat {
            damage: [[0.5, 0.9], [0.0; 2], [0.0; 2], [0.0; 2]],
            ..attacker
        };
        assert_eq!(strike(&weak, &attacker, 1.0, &mut rng), Some(15));
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
        assert_eq!(level(&start, None), 1);
        assert_eq!(level(&west, None), 25);
        let mut rng = Rng::of(50, 39, 0);
        assert!((0..200).all(|_| (1..=2).contains(&level(&start, Some(&mut rng)))));
        assert!((0..200).all(|_| (24..=27).contains(&level(&west, Some(&mut rng)))));
    }
}
