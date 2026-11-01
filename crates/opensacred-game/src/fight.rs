//! The fight as the game plays it (docs/re/fight.md): the creatures' values, health and
//! regeneration as components, the hero's experience and the balance's values as resources, the
//! systems that resolve the hits, regenerate the creatures and raise the hero's level, and the
//! damages' numbers over the targets. The formulas are the rules' (`opensacred_rules::fight`).

use bevy::prelude::*;
use opensacred_data::tables::{Balance, Creature};
pub use opensacred_rules::fight::{
    BOSS_TAG, HERO_LEVEL, Holds, ItemValues, Raise, Rank, Templates, combat, creature_earns,
    experience_with, fill_level, filled_level, hero_health, hero_health_with, hero_skills,
    level_start, most_health, regeneration, scripted_level, skills, tagged_level, takes_on,
};
use opensacred_rules::fight::{
    Side, art_landed, poisons, range_factor, strike_landed, sure_landed, weapons_factor, weighed,
};
use opensacred_rules::potions;

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
    /// Fixed damages of the four kinds in place of the attacker's (the event 5's, as the
    /// Vampiress's claw jump and the Seraphim's bolts send them).
    pub fixed: Option<[f32; 4]>,
    /// Whether it lands without the hit chance's roll (the event 5 of the Gladiator's 1016's
    /// effect and of the Seraphim's bolts; seen, #854).
    pub sure: bool,
    /// Whether it is a poison's own strike, the event 5 of the kind 17 (`0x5607C3`), which
    /// poisons no more (`0x56DFB3`; #1354).
    pub poison: bool,
}

/// The dice of the strikes.
#[derive(Resource)]
pub(crate) struct Dice(Rng);

/// The Dwarf's creature type (docs/02 §6.2).
const DWARF: u32 = 8;
/// The sound of the hero's level-up (`LEVELUP01`).
const LEVEL_UP_SOUND: u32 = 7002;

/// The hero's experience, its level and the points its levels gave
/// (`opensacred_rules::fight::Experience`).
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq, Deref, DerefMut)]
pub struct Experience(pub opensacred_rules::fight::Experience);

/// The balance's values that the fight takes (`opensacred_rules::fight::Rules`).
#[derive(Resource, Clone, Debug, Default, PartialEq, Deref)]
pub struct Rules(pub opensacred_rules::fight::Rules);

impl Rules {
    /// The rules of the balance `balance` on `difficulty` (#1086), the original's values where it
    /// lacks them.
    pub fn at(balance: Option<&Balance>, difficulty: opensacred_rules::drops::Difficulty) -> Self {
        Self(opensacred_rules::fight::Rules::at(balance, difficulty))
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

/// The Dwarf's frenzy (the creature's float `+0x584`): with the effect of his 1082 his strikes
/// raise it and it falls by the frame, and it quickens his attack (`opensacred_rules::arts::
/// quickened`); 0 without the effect.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Frenzy(pub f32);

/// The Vampiress's bloodlust (the creature's float `+0x584`, as the Dwarf's [`Frenzy`]): her
/// strikes in her night form raise it and draw more health by it (`opensacred_rules::arts::
/// bloodlust_damage`), by day they set it to 0, and it falls by the frame.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Bloodlust(pub f32);

/// A creature's rank, a fill's boss or champion (`opensacred_rules::fight::Rank`).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ranked(pub Rank);

/// A boss, a champion or a creature of a champion's levelling not hostile to the hero, whose
/// levelling waits for its first strike or hit (`0x56B130`, `0x550E50`, #630): the hero's look
/// levels the hostile ones by the hero's level (`0x547377`), which the game does as it makes them
/// (docs/decisions/fight.md, "Bosses and champions levelled when made"), but not the others, which
/// take their first opponent's; and whether its type is big (`Creature.pak` flag `0x02`), which its
/// sector's highest level does not hold (`npcs::first_fight`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unfought {
    pub big: bool,
}

/// What a creature is worth: `Creature.pak`'s experience `a + E·b` (`+0x08`, `+0x0A`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Worth(pub [u16; 2]);

/// The experience of a creature that is no hero (its stats' `+0x0C`), which its kills raise
/// (`npcs::earn`, #690): the start of its level when the level's setter last set it
/// (`opensacred_rules::fight::level_start`), so that it is kept only once the creature kills.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Points(pub u64);

/// A creature killed by another that is no hero (#690): the killer, which earns the victim's
/// experience (the event `0x10D`, `0x5311F5`), and what the victim was worth, its level and its
/// rank.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kill {
    pub killer: Entity,
    pub victim: Entity,
    pub worth: Worth,
    pub level: u32,
    pub rank: Rank,
}

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
/// its experience (the event 9, `0x57D860`), a creature that killed it through a [`Kill`].
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
pub(crate) fn resolve(
    mut commands: Commands,
    mut hits: MessageReader<Hit>,
    mut kills: MessageWriter<Kill>,
    mut lines: MessageWriter<crate::voices::Speak>,
    mut blows: MessageWriter<crate::sfx::Blow>,
    mut dice: ResMut<Dice>,
    mut experience: ResMut<Experience>,
    (rules, mut last_foe, mut tally, mut records): (
        Res<Rules>,
        Option<ResMut<crate::statistics::LastFoe>>,
        Option<ResMut<crate::statistics::Tally>>,
        Option<ResMut<crate::statistics::Kills>>,
    ),
    (hero_arts, effects, clock, mut lusts): (
        Option<Res<crate::arts::HeroArts>>,
        Query<&crate::effects::Effects>,
        Option<Res<crate::daynight::Clock>>,
        Query<&mut Bloodlust>,
    ),
    attackers: Query<(
        &Combat,
        Has<Body>,
        Option<&crate::ai::Mind>,
        &GlobalTransform,
        (Option<&Ranked>, Option<&crate::followers::Follower>),
    )>,
    mut targets: Query<
        (
            &mut Health,
            &Combat,
            &GlobalTransform,
            Option<&Worth>,
            Has<Body>,
            Option<&Ranked>,
            (Has<Immortal>, Option<&crate::ai::Mind>),
        ),
        Without<Dies>,
    >,
) {
    for hit in hits.read() {
        let (
            Ok((attacker, hero, mind, from, (ranked, follower))),
            Ok((mut health, defender, at, worth, hero_hit, rank, (immortal, victim))),
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
        let (mut attacker, mut defender, mut blow) =
            weighed(attacker.0, defender.0, side, (level, health.level), &rules);
        // The Dwarf's steel pierces what he strikes: its physical resistance over the effect's
        // value over 100 (`0x56C723`).
        let pierces = effects.get(hit.attacker).ok().and_then(|held| {
            held.timed()
                .iter()
                .find(|timed| timed.effect.id == opensacred_rules::spells::PIERCES)
                .map(|timed| timed.effect.value)
        });
        if let Some(pierces) = pierces.filter(|&value| value > 0.0) {
            defender.resistance[0] *= 100.0 / pierces;
        }
        if hit.ranged {
            let distance = (from.translation() - at.translation()).xz().length();
            let factor = range_factor(distance, &rules);
            attacker.attack *= factor;
            debug!(
                "fight: {} at range {distance:.0} of {}, its attack times {factor:.2}",
                hit.attacker, hit.target
            );
        }
        if let Some(fixed) = hit.fixed {
            attacker.damage = fixed.map(|damage| [damage; 2]);
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
        let landed = match (hit.sure, hit.art) {
            (true, _) => Some(sure_landed(&attacker, &defender, blow, &mut dice.0)),
            (false, Some(art)) => art_landed(&attacker, &defender, blow, art, &mut dice.0),
            (false, None) => strike_landed(&attacker, &defender, blow, &mut dice.0),
        };
        // At the target, the striker's weapon's hit or parry (`0x5320B2`, #712).
        blows.write(crate::sfx::Blow::Target {
            striker: hit.attacker,
            target: hit.target,
            landed: landed.is_some_and(|landed| landed.damage > 0),
        });
        let Some(landed) = landed else {
            debug!("fight: {} misses {}", hit.attacker, hit.target);
            continue;
        };
        // Its poison, where it passes the target's resistance (`0x56DFC8`, #1354); none of the
        // poison's own strikes, of the kind 17.
        if !hit.poison
            && let Some(poison) = poisons(&landed, &mut dice.0)
        {
            let source = hit.attacker;
            debug!(
                "fight: {source} poisons {}: slowed by {}, {:.2} for {:.2} s",
                hit.target, poison.whole, poison.value, poison.seconds
            );
            commands
                .entity(hit.target)
                .entry::<crate::effects::Effects>()
                .or_default()
                .and_modify(move |mut effects| {
                    effects.poison(poison, source);
                });
        }
        let damage = landed.damage;
        // The Daemon's call of death: his strike's damage times its value while above 1
        // (`0x56D1EC`).
        let doom = effects.get(hit.attacker).ok().and_then(|held| {
            held.timed()
                .iter()
                .find(|timed| timed.effect.id == opensacred_rules::spells::DOOMED)
                .map(|timed| timed.effect.value)
        });
        let damage = match doom.filter(|&value| value > 1.0) {
            Some(value) => whole_damage(damage, value),
            None => damage,
        };
        // The Vampiress's strike in her night form raises her bloodlust, by day sets it to 0
        // (the sectors of `0x63D180` that keep it by day are left out), and draws more health by
        // it (`0x56DB08`).
        let damage = if hero
            && hero_arts
                .as_ref()
                .is_some_and(|arts| arts.kind == opensacred_rules::arts::VAMPIRESS_BY_NIGHT)
        {
            let was = lusts.get(hit.attacker).map_or(0.0, |lust| lust.0);
            let by_day = clock
                .as_ref()
                .is_some_and(|clock| clock.phase() == crate::daynight::Phase::Day);
            let lust = if by_day {
                0.0
            } else {
                opensacred_rules::arts::bloodlust_rises(was)
            };
            match lusts.get_mut(hit.attacker) {
                Ok(mut kept) => kept.0 = lust,
                Err(_) => {
                    commands.entity(hit.attacker).insert(Bloodlust(lust));
                }
            }
            let drawn = opensacred_rules::arts::bloodlust_damage(damage, lust);
            debug!("fight: the bloodlust {lust:.2} takes the strike from {damage} to {drawn}");
            drawn
        } else {
            damage
        };
        if !immortal {
            health.current = health.current.saturating_sub(damage);
        }
        // The Dwarf's landed strike raises his frenzy while his 1082's effect holds
        // (`0x56DC31`).
        if hero
            && hero_arts.as_ref().is_some_and(|arts| arts.kind == DWARF)
            && let Some(whole) = effects.get(hit.attacker).ok().and_then(|effects| {
                effects
                    .list()
                    .into_iter()
                    .find(|effect| effect.id == opensacred_rules::arts::FRENZY)
                    .map(|effect| effect.whole)
            })
        {
            commands
                .entity(hit.attacker)
                .entry::<Frenzy>()
                .or_default()
                .and_modify(move |mut frenzy| {
                    frenzy.0 = opensacred_rules::arts::frenzy_rises(frenzy.0, whole);
                    debug!("fight: the frenzy rises to {:.2}", frenzy.0);
                });
        }
        debug!(
            "fight: {} hits {} for {damage}, health {}/{}",
            hit.attacker, hit.target, health.current, health.most
        );
        // A hit that did damage makes its target speak, unless it falls (`0x532161`, #711).
        if damage > 0 && health.current > 0 {
            lines.write(crate::voices::Speak::hit(
                hit.target,
                health.current,
                health.most,
            ));
        }
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
            let by_creature = !hero && !hero_hit;
            // A magic skeleton or an undead goblin may rise again, a skeleton draining a lower
            // killer (`0x532BD6`, #1199): no kill.
            let rank_of = rank.map_or(Rank::Common, |rank| rank.0);
            let plain = hit.art.is_none() && !hit.ranged;
            // The killer's potions' effects: the purple's banishes the undead, the blue's adds
            // to the experience (#1353).
            let killer = effects
                .get(hit.attacker)
                .map(|effects| effects.list())
                .unwrap_or_default();
            let banished = potions::banishes(&killer);
            let rises = victim.and_then(|victim| {
                crate::undead::rolls(&mut dice.0, victim, rank_of, (plain, banished)).inspect(
                    |_| {
                        crate::undead::drain(
                            &mut commands,
                            (hit.attacker, hit.target),
                            (victim.creature, health.level),
                            level,
                        );
                    },
                )
            });
            if let Some(&worth) = worth.filter(|_| by_creature && rises.is_none()) {
                kills.write(Kill {
                    killer: hit.attacker,
                    victim: hit.target,
                    worth,
                    level: health.level,
                    rank: rank.map_or(Rank::Common, |rank| rank.0),
                });
            }
            // The foe the hero or its follower kills, which the statistics window's last foe and
            // hit chance and the statistics tab show (the player's statistics' record,
            // `0x424250`, made where the killer is the player's hero or its master is,
            // `0x5348BA`; **seen**: kept at the kill, with its most health, #1525).
            let recorder = if hero {
                (!hero_hit).then_some(attacker)
            } else {
                follower
                    .and_then(|follower| attackers.get(follower.master).ok())
                    .filter(|&(_, master_hero, ..)| master_hero && !hero_hit)
                    .map(|(combat, ..)| combat.0)
            };
            if let Some(hero_combat) = recorder {
                let foe = crate::statistics::Foe::of(
                    victim.map_or(0, |mind| mind.creature),
                    &health,
                    &defender,
                    rules.creatures_aw(),
                );
                if let Some(last_foe) = last_foe.as_deref_mut() {
                    last_foe.0 = Some(foe);
                }
                // The kills' records, the hero's chances of the kill's moment (#1612).
                if let Some(records) = records.as_deref_mut() {
                    let chances = (
                        crate::statistics::chance(hero_combat.attack, foe.defence),
                        crate::statistics::chance(foe.attack, hero_combat.defence),
                    );
                    records.record(foe, chances);
                }
                // The kills' counter of the same record (`+0x56F0`, #1611).
                if let Some(tally) = tally.as_deref_mut() {
                    tally.killed();
                }
            }
            let worth = worth
                .filter(|_| hero)
                .map(|worth| (*worth, rank.copied(), potions::mentored(&killer)));
            fell(
                &mut commands,
                (&mut dice, &mut experience, &rules),
                (hit.target, by_creature, rises),
                &health,
                worth,
            );
        }
    }
}

/// `damage` times `factor`, taken whole.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a strike's damage, whole and not negative, far below the integers' range"
)]
fn whole_damage(damage: u32, factor: f32) -> u32 {
    (f64::from(damage) * f64::from(factor)).max(0.0) as u32
}

/// Fells `target`, at `health`: it dies, one of its two dying clips at random, and where the
/// hero killed it, of `worth`, its rank and the hero's bonus (`potions::mentored`), the hero
/// earns its experience (the event 9, `0x57D860`), twice for a boss and 1.5 times for a
/// champion; the experience earned. A creature
/// killed by another (`by_creature`, #630) drops nothing, and its killer earns its experience
/// through the [`Kill`] that [`resolve`] sends (`npcs::earn`, #690). An undead that `rises`
/// gives nothing (`undead`, #1199), and a skeleton warrior that rises falls as the body of the
/// type it changes into (`undead::change`, #1260).
fn fell(
    commands: &mut Commands,
    (dice, experience, rules): (&mut Dice, &mut Experience, &Rules),
    (target, by_creature, rises): (Entity, bool, Option<crate::undead::Rises>),
    health: &Health,
    worth: Option<(Worth, Option<Ranked>, f32)>,
) -> u32 {
    let choice = dice.0.below(2);
    let mut dies = if by_creature {
        Dies::by_creature(choice)
    } else {
        Dies::new(choice)
    };
    dies.rises = rises.is_some();
    commands.entity(target).insert(dies);
    let Some(rises) = rises else {
        return earn(experience, rules, target, health, worth);
    };
    info!("fight: {target} falls and will rise");
    if rises.changes_into().is_some() {
        commands.queue(move |world: &mut World| {
            crate::undead::change(world, target, rises, (choice, by_creature));
        });
    } else {
        commands.entity(target).insert(rises);
    }
    0
}

/// The experience the hero earns of `target`, at `health`, of `worth`, its rank and the hero's
/// bonus ([`fell`]).
fn earn(
    experience: &mut Experience,
    rules: &Rules,
    target: Entity,
    health: &Health,
    worth: Option<(Worth, Option<Ranked>, f32)>,
) -> u32 {
    let Some((worth, rank, bonus)) = worth else {
        return 0;
    };
    let earned = experience_with(worth.0, health.level, experience.level, rules, bonus);
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
/// killed them (the command `kill`, #524): they die, drop and give the hero their experience, and
/// count among its kills (#1611); a line for each.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
pub fn kill(
    In(radius): In<u32>,
    mut commands: Commands,
    mut dice: ResMut<Dice>,
    mut experience: ResMut<Experience>,
    (rules, mut tally, mut records): (
        Res<Rules>,
        Option<ResMut<crate::statistics::Tally>>,
        Option<ResMut<crate::statistics::Kills>>,
    ),
    hero: Query<
        (
            &Transform,
            Option<&crate::effects::Effects>,
            Option<&Combat>,
        ),
        With<Body>,
    >,
    mut creatures: Query<
        (
            Entity,
            &Transform,
            &mut Health,
            &crate::ai::Mind,
            Option<&Worth>,
            (Option<&Ranked>, &Combat),
        ),
        (Without<Dies>, Without<Body>),
    >,
) -> Result<Vec<String>, String> {
    let (at, effects, hero_combat) = hero
        .iter()
        .next()
        .ok_or("the hero is not in the world yet")?;
    let at = at.translation;
    let effects = effects.map(|effects| effects.list()).unwrap_or_default();
    let (banished, bonus) = (potions::banishes(&effects), potions::mentored(&effects));
    let home = crate::paths::cell_of(at.x, at.z);
    let radius = i32::try_from(radius).unwrap_or(i32::MAX);
    let mut lines = Vec::new();
    for (entity, transform, mut health, mind, worth, (rank, combat)) in &mut creatures {
        let here = transform.translation;
        let cell = crate::paths::cell_of(here.x, here.z);
        if !mind.hostile() || (cell.0 - home.0).abs() > radius || (cell.1 - home.1).abs() > radius {
            continue;
        }
        health.current = 0;
        let rises = crate::undead::rolls(
            &mut dice.0,
            mind,
            rank.map_or(Rank::Common, |rank| rank.0),
            (true, banished),
        );
        let worth = worth.map(|worth| (*worth, rank.copied(), bonus));
        let earned = fell(
            &mut commands,
            (&mut dice, &mut experience, &rules),
            (entity, false, rises),
            &health,
            worth,
        );
        if let Some(tally) = tally.as_deref_mut() {
            tally.killed();
        }
        if let (Some(records), Some(hero)) = (records.as_deref_mut(), hero_combat) {
            let foe =
                crate::statistics::Foe::of(mind.creature, &health, combat, rules.creatures_aw());
            let chances = (
                crate::statistics::chance(hero.attack, foe.defence),
                crate::statistics::chance(foe.attack, hero.defence),
            );
            records.record(foe, chances);
        }
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
/// refilled; a rise sounds `LEVELUP01` in the middle (`0x57E160` at `0x57EE84`, the event
/// `(9, 0x10)`; #1167).
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
    mut sounds: MessageWriter<crate::sfx::Play>,
) {
    for (base, holding, mut values, mut health, mut regenerates) in &mut heroes {
        let level = experience.level;
        if health.level == level {
            continue;
        }
        if level > health.level {
            sounds.write(crate::sfx::Play {
                sound: crate::sfx::Of::Id(LEVEL_UP_SOUND),
                at: None,
                rate: (0.5, crate::sfx::Roll::None),
            });
        }
        let holds = holding.map(|holding| holding.0.clone()).unwrap_or_default();
        let skills = hero_skills(base, level, &holds.items);
        *values = Combat(combat(base, level, holds, &skills, &rules));
        let most = hero_health_with(base.attributes, base.extras, level, &skills, &rules);
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
    cameras: Query<(&Camera, &GlobalTransform), crate::iso::Eye>,
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
            .add_message::<Kill>()
            .insert_resource(Dice(crate::seed::dice(0)))
            .init_resource::<Experience>()
            .init_resource::<Rules>()
            .add_systems(
                OnEnter(AppState::InGame),
                |mut experience: ResMut<Experience>| *experience = Experience::default(),
            )
            .add_systems(Update, (resolve, level_up, rise).chain())
            .add_systems(Update, calm.in_set(crate::frame::Unpaused))
            .add_systems(
                Update,
                regenerate
                    .in_set(crate::frame::Unpaused)
                    .after(crate::frame::Step::Tick)
                    .run_if(crate::frame::ticked),
            );
    }
}

/// Lowers each frenzy by its effect's value a second, a fifth of it while its creature fights a
/// foe (`0x537E57`), and sets it to 0 without the effect; and each bloodlust by 5 a second, or 1
/// (`0x537DEC`).
fn calm(
    time: Res<Time>,
    mut frenzies: Query<(
        &mut Frenzy,
        Option<&crate::effects::Effects>,
        Option<&crate::attack::Attack>,
    )>,
    mut lusts: Query<(&mut Bloodlust, Option<&crate::attack::Attack>), Without<Frenzy>>,
) {
    let dt = time.delta_secs();
    for (mut lust, attack) in &mut lusts {
        let fighting = attack.is_some_and(|attack| attack.target().is_some());
        let cooler = opensacred_rules::arts::frenzy_falls(
            lust.0,
            opensacred_rules::arts::BLOODLUST_FALLS,
            dt,
            fighting,
        );
        if cooler != lust.0 {
            lust.0 = cooler;
        }
    }
    for (mut frenzy, effects, attack) in &mut frenzies {
        let value = effects.and_then(|effects| {
            effects
                .list()
                .into_iter()
                .find(|effect| effect.id == opensacred_rules::arts::FRENZY)
                .map(|effect| effect.value)
        });
        let fighting = attack.is_some_and(|attack| attack.target().is_some());
        let calmer = value.map_or(0.0, |value| {
            opensacred_rules::arts::frenzy_falls(frenzy.0, value, dt, fighting)
        });
        if calmer != frenzy.0 {
            frenzy.0 = calmer;
        }
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
            multipliers: [1.0; 4],
            resistance_factors: [1.0; 4],
            spells: [1.0; 4],
            cast: opensacred_rules::fight::CAST_SPEED,
            attributes: [0.0; 6],
        });
        let health = Health {
            current: 1_000_000,
            most: 1_000_000,
            level: 1,
        };
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<Hit>()
            .add_message::<Kill>()
            .add_message::<crate::voices::Speak>()
            .add_message::<crate::sfx::Blow>()
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
                    fixed: None,
                    sure: false,
                    poison: false,
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
