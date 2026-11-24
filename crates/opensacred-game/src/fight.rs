//! The fight as the game plays it (docs/re/fight.md): the creatures' values, health and
//! regeneration as components, the hero's experience and the balance's values as resources, the
//! systems that resolve the hits and the strikes from nobody, regenerate the creatures and raise
//! the hero's level, the terror of the hero's items, which fells the creatures its look takes
//! (#1125), the Dwarf's frenzy and the Vampiress's bloodlust, the command `kill`, and the
//! damages' numbers over the targets. The formulas are the rules' (`opensacred_rules::fight`).

use bevy::prelude::*;
use opensacred_data::tables::{Balance, Creature};
pub use opensacred_rules::fight::{
    BOSS_TAG, HERO_LEVEL, Holds, ItemValues, Played, Raise, Rank, Templates, combat,
    creature_earns, experience_with, fill_level, filled_level, hero_health_held, hero_skills,
    level_start, most_health, most_health_held, regeneration, scripted_level, skills, tagged_level,
    takes_on,
};
use opensacred_rules::fight::{
    HurledAt, Side, against_class, armour_comment, art_landed, poisons, range_factor,
    strike_landed, sure_landed, weapon_comment, weapons_factor, weighed,
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
    /// Whether it is the strike of a spell's object that asks its target's magic defence first
    /// (`0x561860`, the sets' M17, #2277; `opensacred_rules::spells::warded`), which may turn it
    /// aside whole.
    pub ward: bool,
    /// Whether it is the hit of a hurled item (`hurled`, the Gladiator's 1013): an event of no
    /// fixed damages whose resolution puts the item's own damages, [`Self::fixed`], in the place
    /// of the four that the attacker's attack record rolled (`0x56B130` at `0x56BC81`), so that
    /// they take neither the record's raises (the survival's, `0x56A50F`) nor the fixed damages'
    /// (`0x56C156`), the hit's roll that of the attacker's own strike.
    pub hurled: bool,
    /// Whether it is the strike of an effect's object, a spell's or another FX object's
    /// (`0x5ABD80` at `0x5ABE3F`–`0x5ABE5D`), whose record names the object for its weapon, of the
    /// category 12: it plays no sound at its target and calls no line of the struck one
    /// (`0x5320D2`, `0x532745`; docs/re/sound.md, "The weapons", #3471).
    pub object: bool,
    /// Whether it is the strike of the horse that its attacker rides, its trample's (`0x597B3C`):
    /// a kill of it is the horse's, its experience worked out at the horse's level, which the
    /// horse and its rider each take (docs/re/fight.md, "The mount's and the rider's share",
    /// #3586).
    pub mount: bool,
}

/// A strike from nobody, an attacker's id of 0: a fire smoke's or a bursting geyser's (`fx`,
/// #2518), a ghost's of the scripts' field (`pill_fields`), a fence's of lightning
/// (`laser_fences`, #2916) or a slave's of Shaddar's (`shaddar_slaves`, #2919), of fixed
/// damages, sure ([`burn`]).
#[derive(Message, Clone, Copy, Debug)]
pub struct Burn {
    pub target: Entity,
    pub damages: [f32; 4],
}

/// A creature slain outright, its health set to 0 and its AI's state to 9, dead, with no strike:
/// the zhadum's kill (`zhadums`, `0x5B2285`; [`slay`]).
#[derive(Message, Clone, Copy, Debug)]
pub struct Slay {
    pub target: Entity,
}

/// The dice of the strikes.
#[derive(Resource)]
pub(crate) struct Dice(Rng);

/// The Dwarf's creature type (docs/02 §6.2).
const DWARF: u32 = 8;
/// The sound of the hero's level-up (`LEVELUP01`).
const LEVEL_UP_SOUND: u32 = 7002;
/// The sound of a life leech, at the struck one (`SPELL_LASER`, `0x570BB7`).
const LEECH_SOUND: u32 = 2074;
/// The magic lore (`MK…`), the skill by which a caster's level weighs against a magic defence.
const MAGIC_LORE: u8 = 14;

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
#[derive(
    Component,
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Deref,
    serde::Serialize,
    serde::Deserialize,
)]
// Named apart from another saved type of its name, which the saves' layout test traces by
// name (#2306); postcard writes no names.
#[serde(rename = "CreatureBase")]
pub struct Base(pub opensacred_rules::fight::Base);

impl Base {
    pub fn of(creature: &Creature) -> Self {
        Self(opensacred_rules::fight::Base::of(creature))
    }
}

/// What the hero holds and wears, a component of the hero's, which its level-up takes: its
/// template's items as it is made (`npcs::Npcs::hero_holds`), then the items in its hands and in
/// its equipment's slots as they change (`arsenal`, `equipment::refit`).
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

/// A strike resolved at its target (`0x56B130`, the creature events' handler `0x52E590`'s event
/// 5): its target, the damage it did, 0 for a miss or a damage resisted whole, and the target's
/// most health, which the blows' sights take (`blow_sights`: the bleed and the parade).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Struck {
    pub target: Entity,
    pub damage: u32,
    pub most: u32,
}

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

/// A player's hero knocked down by a heavy blow (`0x56E366`–`0x56E8CC`, #2311): the seconds it
/// still lies, where its attacker struck from, and how far its fall has thrown it
/// (`knock_down`).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Downed {
    pub left: f32,
    pub from: Vec3,
    pub thrown: f32,
}

/// A player's hero knocked back by a whirlwind's strike (the event of the kind 3, `0x531AF7`,
/// #2901): the place of the whirlwind's caster, the event's sender (#3305), and how far it still
/// slides (`+0x24D`), which each new knock sets anew (`knock_down::slide`).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Pushed {
    pub from: Vec3,
    pub left: f32,
}

/// How many times a hero was knocked down since its marks were cleared (the stats' `+0x2B7`
/// bits `0x80` and `0x100`), which its whole health in a settlement clears (`0x54808A`).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KnockMarks(pub u8);

/// A creature that takes no damage, for a test (the commands `spawn --immortal` and `god`,
/// #521): a hit lands and shows its damage, but the health stays.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Immortal;

/// The scripts' mark to rise again on a struck creature (`+0x2B7` bit 8, `npcs::RisesAgain`;
/// docs/re/ai.md, "The scripts' hired followers"): whether it bears the mark, a hero's strike or
/// another's (`hero`), on Sharifa or another (`sharifa`), of a hero for its master, of a dialogue,
/// and of a function of the scripts but an item's and a sight's: the mark it bears after the
/// strike, and whether the strike leaves its health. A hero's strike clears the mark from one of
/// no dialogue, no such function and no hero for its master (`0x56B296`–`0x56B306`); Sharifa
/// takes it at every strike (`0x56B316`); and with it and no hero for its master the health's fall
/// is skipped (`0x56F859`).
fn marked_after(
    marked: bool,
    (hero, sharifa): (bool, bool),
    (mastered, talks, functions): (bool, bool, bool),
) -> (bool, bool) {
    let cleared = marked && hero && !talks && !functions && !mastered;
    let marked = (marked && !cleared) || sharifa;
    (marked, marked && !mastered)
}

/// Resolves each hit (`0x56B130`), weighed by the difficulty between the hero and a creature
/// ([`weighed`]), a hit at range with its attack times the range's factor of the distance between
/// the attacker and the target ([`range_factor`], `0x56B449`), and a creature's at the hero, but a
/// boss's or a champion's, with its damage times the factor of its level ([`weapons_factor`],
/// `0x56A890`): a target without a health, or dying, ignores it; else the strike may miss, and
/// its damage takes the target's health down, but for one that the scripts' mark spares
/// ([`marked_after`]), shows over it, and at 0 the target dies (the AI's
/// state 9, `0x53E7C0`), one of its two dying clips at random, and a hero that killed it earns
/// its experience (the event 9, `0x57D860`), the hero's follower's kill too, a creature that
/// killed it through a [`Kill`].
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
    (mut blows, mut remarks, mut struck): (
        MessageWriter<crate::sfx::Blow>,
        MessageWriter<crate::remarks::Remark>,
        MessageWriter<Struck>,
    ),
    mut dice: ResMut<Dice>,
    mut experience: ResMut<Experience>,
    (rules, mut last_foe, mut tally, mut records, spared): (
        Res<Rules>,
        Option<ResMut<crate::statistics::LastFoe>>,
        Option<ResMut<crate::statistics::Tally>>,
        Option<ResMut<crate::statistics::Kills>>,
        Query<&crate::making::Making, With<crate::making::Playing>>,
    ),
    (hero_arts, effects, (clock, dungeons), mut lusts): (
        Option<Res<crate::arts::HeroArts>>,
        Query<&crate::effects::Effects>,
        (
            Option<Res<crate::daynight::Clock>>,
            Option<Res<crate::daynight::Dungeons>>,
        ),
        Query<&mut Bloodlust>,
    ),
    attackers: Query<(
        &Combat,
        Has<Body>,
        Option<&crate::ai::Mind>,
        &GlobalTransform,
        (
            Option<&Ranked>,
            Option<&crate::followers::Follower>,
            Option<&Holding>,
            Option<&crate::npcs::Arms>,
            Option<&crate::riding::Seat>,
        ),
    )>,
    mut targets: Query<
        (
            &mut Health,
            &Combat,
            &GlobalTransform,
            Option<&Worth>,
            Has<Body>,
            Option<&Ranked>,
            (
                Has<Immortal>,
                Option<&crate::ai::Mind>,
                Option<&Holding>,
                Option<&crate::npcs::Arms>,
                Option<&crate::riding::Seat>,
                (
                    Option<&Downed>,
                    Option<&KnockMarks>,
                    Option<&crate::npcs::ScriptClip>,
                    Option<&crate::hero_defence::Defending>,
                ),
                (
                    Has<crate::followers::Follower>,
                    Has<crate::npcs::RisesAgain>,
                    Has<crate::npcs::Talk>,
                    Option<&crate::npcs::OnDeath>,
                    Has<crate::undead::Marked>,
                ),
            ),
        ),
        Without<Dies>,
    >,
    played: Option<Res<crate::drops::Played>>,
    (difficulty, mut pack, mut piles, mut sounds): (
        Option<Res<crate::game::Difficulty>>,
        Option<ResMut<crate::pickup::Pack>>,
        MessageWriter<crate::drops::PileFromHit>,
        MessageWriter<crate::sfx::Play>,
    ),
) {
    let difficulty = difficulty.map_or_else(Default::default, |difficulty| difficulty.0);
    for hit in hits.read() {
        // An event of fixed damages whose whole four sum to 0 is its attacker's own strike, its
        // damages rolled and its hit chance asked (`0x56B130` at `0x56B1B9`; #1856): the big
        // demons' laser and fire. A hurled item's damages are its own, whatever they sum to.
        let hit = &match hit.fixed {
            Some(fixed) if !hit.hurled && fixed.iter().all(|damage| damage.trunc() == 0.0) => Hit {
                fixed: None,
                sure: false,
                ..*hit
            },
            _ => *hit,
        };
        // The attacker's own health, which its wounds' damage takes (`0x549460`).
        let attacker_health = targets
            .get(hit.attacker)
            .ok()
            .map(|(health, ..)| (health.current, health.most));
        // The level of the horse whose strike it is, its trample's, which its kill's experience
        // is worked out at (`0x597B3C`, `0x57D860`; #3586).
        let mount_level = hit
            .mount
            .then(|| {
                let (.., (_, _, _, _, seat)) = attackers.get(hit.attacker).ok()?;
                let horse = seat.and_then(crate::riding::Seat::ridden)?;
                targets.get(horse).ok().map(|(health, ..)| health.level)
            })
            .flatten();
        let (
            Ok((attacker, hero, mind, from, (ranked, follower, holding, caster_arms, saddle))),
            Ok((
                mut health,
                defender,
                at,
                worth,
                hero_hit,
                rank,
                (
                    immortal,
                    victim,
                    guarded,
                    arms,
                    seat,
                    (downed, knock_marks, clip, hero_defends),
                    (followed, rises, talks, on_death, undead_marked),
                ),
            )),
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
        // A maker of a jump, of the Seraphim's 1018 or 1019 or of the backbreaker under way holds
        // the mark that a strike passes by (`0x56B130`, `+0x200` bit `0x10`; #3126).
        if spared
            .get(hit.target)
            .is_ok_and(|making| opensacred_rules::arts::spares(making.art))
        {
            debug!(
                "fight: {} hits {}, which its art spares",
                hit.attacker, hit.target
            );
            continue;
        }
        let side = match (hero, hero_hit) {
            (true, false) => Some(Side::Attacker),
            (false, true) => Some(Side::Defender),
            _ => None,
        };
        let level = mind.map_or(experience.level, |mind| mind.level);
        // A spell object's strike that the target's magic defence turns aside reaches it not at
        // all (`0x561860`, the sets' M17, #2277): the target's items' 858, the hero's grown by its
        // sets' terms, against the caster's level raised by its magic lore.
        if hit.ward {
            let ward = magic_defence(hero_hit, (guarded, arms), defender.0.attributes);
            let lore = caster_lore((hero, caster_arms, level), hero_arts.as_deref(), &rules);
            let held = effects.get(hit.target).ok();
            if let Some(turned) = turned_aside(
                &mut dice,
                (ward, level, lore),
                held,
                (hit.target, hit.attacker),
            ) {
                // The ward flashes its object (`0x562254`, #3172); the spellproof's shares the
                // spell among the creatures hostile to its bearer too.
                if let Turned::Effect { art, value } = turned {
                    let (bearer, damages) = (hit.target, hit.fixed);
                    commands.queue(move |world: &mut World| {
                        if art == opensacred_rules::spells::SPELLPROOF
                            && let Some(damages) = damages
                        {
                            crate::spells::share_turned(world, bearer, damages, value);
                        }
                        world.write_message(crate::shields::Flash(bearer, art));
                    });
                }
                continue;
            }
        }
        // The scripts' mark to rise again, which a hero's strike may clear and Sharifa takes, and
        // which spares the struck one while no hero is its master (#3294).
        let sharifa =
            victim.is_some_and(|victim| victim.creature == opensacred_rules::casts::SHARIFA);
        let functions = on_death.is_some_and(|on_death| on_death.0.holds());
        let (marked, unharmed) = marked_after(rises, (hero, sharifa), (followed, talks, functions));
        if marked != rises {
            debug!(
                "fight: {} strikes {}, marked {marked}",
                hit.attacker, hit.target
            );
            if marked {
                commands.entity(hit.target).insert(crate::npcs::RisesAgain);
            } else {
                commands
                    .entity(hit.target)
                    .remove::<crate::npcs::RisesAgain>();
            }
        }
        // A defender in its defending clip, a parry's or a script's (the slots 53 to 65), doubles
        // its defence (`0x56B96A`–`0x56B9A5`, #2487); the hero in its own under Shift past its
        // blend, eight times, with its four resistances twice (`0x56C6B4`–`0x56C70F`, #3392).
        let mut defending = defender.0;
        if clip.is_some_and(crate::npcs::ScriptClip::defends) {
            defending.defence *= opensacred_rules::fight::PARRY_DEFENCE;
            debug!("fight: {} defends against {}", hit.target, hit.attacker);
        }
        if hero_defends.is_some_and(|defends| defends.on) {
            defending.defence *=
                opensacred_rules::fight::PARRY_DEFENCE * opensacred_rules::fight::HERO_DEFENCE;
            for resistance in &mut defending.resistance {
                *resistance *= opensacred_rules::fight::HERO_RESISTANCE;
            }
            debug!("fight: the hero defends against {}", hit.attacker);
        }
        // The two's records against each other (`0x568D70`, #3417): the attacker's slots against
        // the struck one's class raise its ends and `A` for a blow of no fixed damages (a hurl's
        // `A` alone), the struck one's against the attacker's its resistances and `D`, but at a
        // poison's own strike, whose record of the struck one is none.
        let hero_type = hero_arts.as_deref().map_or(0, |arts| arts.kind);
        let attacker_type = if hero {
            hero_type
        } else {
            mind.map_or(0, |mind| mind.creature)
        };
        let target_type = if hero_hit {
            hero_type
        } else {
            victim.map_or(0, |victim| victim.creature)
        };
        // The purple potions that either bears, which raise their bearer against an undead and
        // lower an undead against their bearer in both records (`0x569A29`–`0x569D72`, #3483).
        let potions_of = |entity: Entity| {
            effects
                .get(entity)
                .map(|held| potions::banishing(&held.list()))
                .unwrap_or_default()
        };
        let (striker_potions, struck_potions) = (potions_of(hit.attacker), potions_of(hit.target));
        let is_undead = |mind: Option<&crate::ai::Mind>| {
            mind.is_some_and(|mind| mind.class == opensacred_rules::undead::CLASS)
        };
        let (striker_undead, struck_undead) = (is_undead(mind), is_undead(victim));
        let mut striking = attacker.0;
        if hit.fixed.is_none() || hit.hurled {
            let against = HurledAt {
                thrower: attacker_type,
                ..opponent(hero_hit, victim, (guarded, arms, seat))
            };
            let add = against_class(
                &items_of(holding, caster_arms),
                striking.attributes,
                against,
            )
            .with_potions(
                &striker_potions,
                struck_undead,
                &struck_potions,
                striker_undead,
            );
            let raised = striking.striking_with(&add);
            striking = if hit.hurled {
                opensacred_rules::fight::Combat {
                    attack: raised.attack,
                    ..striking
                }
            } else {
                raised
            };
        }
        if !hit.poison {
            let against = HurledAt {
                thrower: target_type,
                ..opponent(hero, mind, (holding, caster_arms, saddle))
            };
            let add = against_class(&items_of(guarded, arms), defending.attributes, against)
                .with_potions(
                    &struck_potions,
                    striker_undead,
                    &striker_potions,
                    struck_undead,
                );
            defending = defending.struck_with(&add);
        }
        // The two's stored attributes, by which their items' rare effects grow (`0x568C40`), and
        // the struck one as the attacker's opponent in them (`0x570ED0`, #3484).
        let (striker_attributes, struck_attributes) = (striking.attributes, defending.attributes);
        let struck_one = opponent(hero_hit, victim, (guarded, arms, seat));
        let (mut attacker, mut defender, mut blow) =
            weighed(striking, defending, side, (level, health.level), &rules);
        // A poison's own strike makes no record of the struck one against its attacker, which
        // gives the resistances, so that it meets none (`0x56B6AA`, `0x56B929`; docs/re/fight.md,
        // "The poison", #3482).
        if hit.poison {
            defender.resistance = [0.0; opensacred_rules::fight::KINDS];
        }
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
        // The struck hero's sets against a shot: its defence times their factor (`0x56B6D1`,
        // #1727).
        if hit.ranged
            && side == Some(Side::Defender)
            && let Some(guarded) = guarded
        {
            defender.defence *= guarded.0.sets.ranged_defence();
        }
        if hit.ranged {
            let distance = (from.translation() - at.translation()).xz().length();
            let mut factor = range_factor(distance, &rules);
            // From a horse, divided by the skills 7 and 22 that the original reads off the struck
            // one, not the shooter (`0x56B48E`, a quirk; #2815): the hero's with their items'
            // bonuses, a creature's of its type at its level.
            if saddle.and_then(crate::riding::Seat::ridden).is_some() {
                let skill = |skill: u8| {
                    let level = if hero_hit {
                        hero_arts.as_deref().map_or(0, |arts| {
                            arts.skills
                                .iter()
                                .find(|&&(had, _)| had == skill)
                                .map_or(0, |&(_, level)| level)
                        })
                    } else {
                        arms.map_or(0, |arms| arms.skill_level(health.level, skill))
                    };
                    u8::try_from(level).unwrap_or(u8::MAX)
                };
                factor /= opensacred_rules::fight::mounted_range_divisor(skill(7), skill(22));
            }
            attacker.attack *= factor;
            debug!(
                "fight: {} at range {distance:.0} of {}, its attack times {factor:.2}",
                hit.attacker, hit.target
            );
        }
        if let Some(fixed) = hit.fixed {
            attacker.damage = fixed.map(|damage| [damage; 2]);
        }
        // A hurled item's damages are put in after the difficulty's factors, so that a
        // creature's hurl at the hero takes no `ProzDmg` (`0x56BBB8`, `0x56BC81`).
        if hit.hurled {
            blow.rolled = 1.0;
        }
        // The survival bonus of the time played: the hero's four damages, or its strike's fixed
        // ones, raised by half its curve and set 28's part (`0x56A50F`, `0x56C156`; #1761); not a
        // hurled item's, which take the place of the record's raised ones (#3382).
        if side == Some(Side::Attacker) && !hit.hurled {
            let set = holding.map_or(0.0, |holding| holding.0.sets.survival);
            let minutes = played.as_deref().map_or(0, crate::drops::Played::minutes);
            let raise = opensacred_rules::fight::survival_raise(minutes, set);
            attacker.damage = attacker.damage.map(|ends| {
                ends.map(|end| {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "a damage, stored as the attack record's float"
                    )]
                    let raised = (f64::from(end) * raise) as f32;
                    raised
                })
            });
        }
        let ranked = ranked.map_or(Rank::Common, |ranked| ranked.0);
        let factor = weapons_factor(side, level, ranked, &rules);
        blow.share = hit.share;
        blow.weapons = factor;
        // The six sets' last bonuses: the strike's whole damage times the difficulty's factor
        // (`0x56D0C9`, `set_bonuses::WHOLE_DAMAGE`; #1727).
        if let Some(times) = holding.and_then(|holding| holding.0.sets.whole_damage) {
            blow.weapons *= f64::from(times);
        }
        // The hero's sets against the undead, the dragons and the demons: each kind's damage
        // through the resistances (`0x56CA85`…, #1727).
        if side == Some(Side::Attacker)
            && let (Some(holding), Some(victim)) = (holding, victim)
        {
            blow.kinds = holding.0.sets.against(victim.class);
        }
        if side == Some(Side::Defender) {
            debug!(
                "fight: {} of level {level} strikes {}, its damage times {factor:.4}",
                hit.attacker, hit.target
            );
        }
        // The struck hero's gold for its health: its items' effect 866 and its sets' part, held
        // at the difficulty's most (`0x56D805`, #1779); the creatures carry no gold.
        if side == Some(Side::Defender)
            && let Some(guarded) = guarded
            && let Some(pack) = pack.as_deref()
        {
            let value = opensacred_rules::fight::item_effect(
                &guarded.0.items,
                opensacred_rules::fight::GOLD_FOR_HEALTH,
                struck_attributes,
                None,
            );
            let value = guarded
                .0
                .sets
                .gold_for_health(value)
                .min(opensacred_rules::fight::gold_for_health_cap(difficulty));
            if value > 0.0 {
                blow.gold = Some((value, pack.gold));
            }
        }
        // The stun's roll of a strike by the hero's weapon, not an art's shot (`0x56D47C`, #1727's
        // M18): its items' effect 862 and its sets' term. The strikes of fixed damages (the
        // spells' objects, the effects, the hurled blades, the shells) roll none, and nor do the
        // creatures, which wear no item of it (**hypothesis** for the blades, the shells and the
        // creatures' strikes, whose weapons the original tests by their category).
        if hero
            && !hit.sure
            && hit.fixed.is_none()
            && !(hit.ranged && hit.art.is_some())
            && let Some(holding) = holding.filter(|holding| holding.0.weapon)
        {
            let value = opensacred_rules::fight::item_effect(
                &holding.0.items,
                opensacred_rules::fight::STUN,
                striker_attributes,
                None,
            );
            blow.stun = Some((
                holding.0.sets.stun(value),
                rules.rare(opensacred_rules::fight::STUN),
            ));
        }
        // A bonus against the undead marks the undead it fights (`+0x2B7` bit `0x80`, #3385):
        // the attacker's record made against an undead that its strike of no fixed damages
        // strikes, a hurl's among them, before the hit's roll (`0x56B965`; the hurled item's own
        // mark is `hurled`'s), and the struck one's against an undead that strikes it
        // (`0x56B6CC`), but at the poison's own strike.
        let hero_kind = hero_arts.as_deref().map_or(0, |arts| arts.kind);
        let marks = |holding: Option<&Holding>, arms: Option<&crate::npcs::Arms>, kind: u32| match (
            holding, arms,
        ) {
            (Some(holding), _) => {
                opensacred_rules::fight::marks_undead(&holding.0.items, hero_kind)
            }
            (None, Some(arms)) => opensacred_rules::fight::marks_undead(&arms.item_values(), kind),
            (None, None) => false,
        };
        let undead = |mind: Option<&crate::ai::Mind>| {
            mind.is_some_and(|mind| mind.class == opensacred_rules::undead::CLASS)
        };
        let undead_marked = undead_marked
            || ((hit.fixed.is_none() || hit.hurled)
                && undead(victim)
                && marks(holding, caster_arms, mind.map_or(0, |mind| mind.creature)));
        if undead_marked {
            commands.entity(hit.target).insert(crate::undead::Marked);
        }
        if !hit.poison
            && undead(mind)
            && marks(guarded, arms, victim.map_or(0, |victim| victim.creature))
        {
            commands.entity(hit.attacker).insert(crate::undead::Marked);
        }
        let landed = match (hit.sure, hit.art) {
            (true, _) => Some(sure_landed(&attacker, &defender, blow, &mut dice.0)),
            (false, Some(art)) => art_landed(&attacker, &defender, blow, art, &mut dice.0),
            (false, None) => strike_landed(&attacker, &defender, blow, &mut dice.0),
        };
        // At the target, the striker's weapon's hit or parry (`0x5320B2`, #712), none for an
        // object's strike (#3471).
        if !hit.object {
            blows.write(crate::sfx::Blow::Target {
                striker: hit.attacker,
                target: hit.target,
                landed: landed.is_some_and(|landed| landed.damage > 0),
                physical: landed.is_some_and(|landed| landed.through[0] > 0.0),
            });
        }
        // The open wound's rolls of a strike that the struck one lives through, a miss's too
        // (#1727, M15): the struck one's own items' effect 857 for the hero's strike, the
        // attacker's sets' blocks, whether it bleeds already.
        let wound_rolls = (
            (
                arms.map_or(0.0, |arms| {
                    opensacred_rules::fight::item_effect(
                        &arms.item_values(),
                        opensacred_rules::fight::OPEN_WOUNDS,
                        struck_attributes,
                        None,
                    )
                }),
                side == Some(Side::Attacker),
            ),
            holding.map_or(([None; 2], 1.0), |holding| {
                (holding.0.sets.open_wounds(), holding.0.sets.factor)
            }),
            effects
                .get(hit.target)
                .is_ok_and(crate::effects::Effects::bleeds),
        );
        let Some(landed) = landed else {
            debug!("fight: {} misses {}", hit.attacker, hit.target);
            struck.write(Struck {
                target: hit.target,
                damage: 0,
                most: health.most,
            });
            wound(
                &mut commands,
                (hit.target, hit.attacker),
                0,
                wound_rolls,
                &mut dice.0,
            );
            continue;
        };
        // The gold that paid the strike, through the gold adder, which sounds it, and the strike's
        // block sounds it once more (`0x57FD80`, `0x56D9E8`).
        if landed.paid > 0
            && let Some(pack) = pack.as_deref_mut()
        {
            pack.add_gold(-i64::from(landed.paid));
            debug!(
                "fight: {} pays {} gold of {} for its health",
                hit.target, landed.paid, hit.attacker
            );
            sounds.write(crate::pickup::gold_sound());
        }
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
        // The mark that the breath's fires spare, of a wearer of sets 27 or 31, where its fire
        // passes the target's resistance (`0x56E0A4`, #3438); none of the poison's own strikes.
        if !hit.poison
            && holding.is_some_and(|holding| holding.0.sets.marks_for_fire())
            && let Some(seconds) = opensacred_rules::fight::spared_by_fire(&landed)
        {
            let source = hit.attacker;
            debug!(
                "fight: {source} marks {} for the fires {seconds:.2} s",
                hit.target
            );
            commands
                .entity(hit.target)
                .entry::<crate::effects::Effects>()
                .or_default()
                .and_modify(move |mut effects| effects.spare(seconds, source));
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
        // Shaddar's ward: no damage from an attacker that holds no slave's mark (`0x56DAD2`,
        // #2919); its poison still passes.
        let damage = if victim.is_some_and(|victim| {
            opensacred_rules::shaddar::wards(
                victim.creature,
                effects
                    .get(hit.attacker)
                    .is_ok_and(|held| held.holds(opensacred_rules::shaddar::MARK)),
            )
        }) {
            debug!(
                "fight: Shaddar's ward takes {damage} of {}'s strike",
                hit.attacker
            );
            0
        } else {
            damage
        };
        // The Vampiress's strike in her night form raises her bloodlust, by day sets it to 0 but
        // in a dungeon's sector, where it rises as by night (`0x63D180`), and draws more health
        // by it (`0x56DB08`).
        let damage = if hero
            && hero_arts
                .as_ref()
                .is_some_and(|arts| arts.kind == opensacred_rules::arts::VAMPIRESS_BY_NIGHT)
        {
            let was = lusts.get(hit.attacker).map_or(0.0, |lust| lust.0);
            let by_day = clock
                .as_ref()
                .is_some_and(|clock| clock.phase() == crate::daynight::Phase::Day);
            let place = from.translation();
            let dungeon = dungeons
                .as_ref()
                .is_some_and(|dungeons| dungeons.holds(place.x, place.z));
            let lust = if by_day && !dungeon {
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
        // The hero's comment on a strike that its armour did not resist or that its weapon's
        // kind did not drive through (`0x56DE0C`, `0x56DF73`, #1673), by the strike's whole damage.
        let comment = match side {
            Some(Side::Defender) => {
                armour_comment(&landed, damage).map(|kind| crate::remarks::Remark::Armour { kind })
            }
            Some(Side::Attacker) => weapon_comment(&landed, damage, health.most)
                .map(|kind| crate::remarks::Remark::Weapon { kind }),
            None => None,
        };
        if let Some(comment) = comment {
            debug!("fight: the strike makes the hero comment {comment:?}");
            remarks.write(comment);
        }
        // The attacker's rare effects on the whole damage (#1779): its wounds' (867, its items',
        // its sets' and its berserk's per cent, the effect 47's list word,
        // `opensacred_rules::arts::berserk`; `0x56E15D`), then its critical hit's (859 against
        // the struck one, the hero's sets' terms; `0x56E8CC`). Its red flash is not followed.
        let items = holding.map_or(&[][..], |holding| &holding.0.items[..]);
        let berserk = effects.get(hit.attacker).ok().map_or(0.0, |effects| {
            effects
                .list()
                .into_iter()
                .find(|effect| effect.id == opensacred_rules::arts::FRENZY)
                .map_or(0.0, |effect| f32::from(effect.level))
        });
        let wounds = opensacred_rules::fight::item_effect(
            items,
            opensacred_rules::fight::WOUNDS,
            striker_attributes,
            None,
        );
        let wounds = holding.map_or(wounds, |holding| holding.0.sets.wounds(wounds)) + berserk;
        let damage = match attacker_health {
            Some(own) => opensacred_rules::fight::wounded(damage, wounds, own, &rules),
            None => damage,
        };
        // A heavy blow knocks a player's hero down (`0x56E366`–`0x56E8CC`, #2311), outside the
        // god mode: one down takes none; one struck by an attacker more than 4 and a roll levels
        // above it, for at least its health, twice at most until its marks are cleared, falls
        // (`knock_down`), thrown off its horse first, keeping a ninth of its health.
        let damage = if hero_hit && !immortal {
            let marks = knock_marks.map_or(0, |marks| marks.0);
            if downed.is_some() {
                0
            } else if opensacred_rules::fight::knocks_down(
                (level, health.level),
                dice.0.below(2),
                (damage, health.current),
                marks,
            ) {
                info!(
                    "fight: {} knocks {} down, its {} marks",
                    hit.attacker,
                    hit.target,
                    marks + 1
                );
                commands.entity(hit.target).insert((
                    Downed {
                        left: opensacred_rules::fight::KNOCKED_DOWN_SECONDS,
                        from: from.translation(),
                        thrown: 0.0,
                    },
                    KnockMarks(marks + 1),
                ));
                // Its butterflies, white for 6 s (the object 874, `0x56E7D2`; #3023).
                crate::knockouts::knocked_down(&mut commands, hit.target);
                if seat.and_then(crate::riding::Seat::ridden).is_some() {
                    let me = hit.target;
                    commands.queue(move |world: &mut World| crate::riding::throw_off(world, me));
                }
                opensacred_rules::fight::knocked_damage(health.current)
            } else {
                damage
            }
        } else {
            damage
        };
        let critical = opensacred_rules::fight::item_effect(
            items,
            opensacred_rules::fight::CRITICAL,
            striker_attributes,
            Some(struck_one),
        );
        let critical = match holding.filter(|_| hero) {
            Some(holding) => holding.0.sets.critical(critical),
            None => critical,
        };
        // A defender both a boss and of a champion's mark takes the wider roll; the game's
        // ranks are one or the other (**hypothesis**: never both in the original either).
        let parameters = (rules.rare(opensacred_rules::fight::CRITICAL), false);
        let damage =
            match opensacred_rules::fight::critical(damage, critical, parameters, &mut dice.0) {
                Some(critical) => {
                    debug!(
                        "fight: {} strikes {} critically: {damage} to {critical}",
                        hit.attacker, hit.target
                    );
                    // Its red flash at the defender, at every critical hit (`0x56EF58`; docs/
                    // decisions/drawing.md, "A critical hit flashes every time", #3081).
                    let target = hit.target;
                    commands.queue(move |world: &mut World| {
                        crate::group_looks::flash(
                            world,
                            target,
                            opensacred_rules::particles::CRITICAL_FLASH,
                        );
                    });
                    critical
                }
                None => damage,
            };
        // A gold pile that the hit makes the struck one drop: the attacker's items' effect 856 and
        // its sets' part, held at the damage and at the health before the hit (`0x56F679`, #1779).
        let pile = opensacred_rules::fight::item_effect(
            items,
            opensacred_rules::fight::GOLD_PILE,
            striker_attributes,
            None,
        );
        let pile = holding.map_or(pile, |holding| holding.0.sets.gold_pile(pile));
        if let Some((item, most)) =
            opensacred_rules::fight::gold_pile(pile, damage, health.current, &mut dice.0)
        {
            piles.write(crate::drops::PileFromHit {
                creature: hit.target,
                item,
                most,
            });
            sounds.write(crate::pickup::gold_sound());
        }
        // The struck one's health falls by the strike (`0x56F8B9`–`0x56F9AA`), what it held before
        // kept for the leech (`0x56F855`).
        let before = health.current;
        if !immortal && !unharmed {
            health.current = health.current.saturating_sub(damage);
        }
        // A lethal blow on a rider leaves it 1 health and fells its horse instead, whose death
        // throws it off (`0x56F8DB`, #2299).
        if health.current == 0
            && let Some(horse) = seat.and_then(crate::riding::Seat::ridden)
        {
            health.current = 1;
            let choice = dice.0.below(2);
            commands.queue(move |world: &mut World| crate::riding::fell(world, horse, choice));
        }
        // The attacker's effect 842 then raises the damage that the resolution returns by what it
        // pays of its own health, keeping 1 (`0x5706F8`–`0x5707F1`, #1366): the struck one's
        // health has fallen already, so the raise takes none of it, while the wound and the
        // number shown take it, and it flashes red at the attacker where it pays (856,
        // `0x570803`, #3081). The hero's sets add their part to its items' value (M21,
        // `0x570721`).
        let pain = opensacred_rules::fight::item_effect(
            items,
            opensacred_rules::fight::PAINKILLER,
            striker_attributes,
            None,
        );
        let pain = match holding.filter(|_| hero) {
            Some(holding) => holding.0.sets.painkiller(pain),
            None => pain,
        };
        let cost = opensacred_rules::fight::painkiller(damage, pain, &rules);
        if cost > 0 {
            let attacker = hit.attacker;
            commands.queue(move |world: &mut World| {
                crate::group_looks::flash(
                    world,
                    attacker,
                    opensacred_rules::particles::PAINKILLER_FLASH,
                );
            });
        }
        let damage = damage + cost;
        // Its life leech takes back a share of the raised damage, held at what the struck one's
        // health held before, by their distance, but from a poison's strike
        // (`0x5708D3`–`0x570A80`); its beam (the variant 2) goes from its spine to a bone of the
        // struck one's at random, with `SPELL_LASER` at the struck one.
        let life = if hit.poison {
            0.0
        } else {
            opensacred_rules::fight::leech(
                (damage, before),
                (
                    opensacred_rules::fight::item_effect(
                        items,
                        opensacred_rules::fight::LEECH,
                        striker_attributes,
                        None,
                    ),
                    difficulty.leech(),
                ),
                (from.translation() - at.translation())
                    .xz()
                    .length()
                    .round(),
                &rules,
            )
        };
        if cost > 0 || life > 0.0 {
            debug!(
                "fight: {} pays {cost} for its strike and leeches {life:.2}",
                hit.attacker
            );
            commands
                .entity(hit.attacker)
                .entry::<Health>()
                .and_modify(move |mut own| {
                    if own.current == 0 {
                        return;
                    }
                    if cost > 0 {
                        own.current = opensacred_rules::fight::painkiller_paid(own.current, cost);
                    }
                    if life > 0.0 {
                        own.current =
                            opensacred_rules::fight::leeched((own.current, own.most), life);
                    }
                });
        }
        if life > 0.0 {
            let bone = usize::try_from(dice.0.below(50)).unwrap_or(0);
            crate::beams::lay(
                &mut commands,
                (
                    crate::beams::End::spine(hit.attacker),
                    crate::beams::End::bone(hit.target, bone),
                ),
                2,
            );
            sounds.write(crate::sfx::Play {
                sound: crate::sfx::Of::Id(LEECH_SOUND),
                at: Some(hit.target),
                rate: (0.5, crate::sfx::Roll::None),
            });
        }
        if health.current > 0 {
            wound(
                &mut commands,
                (hit.target, hit.attacker),
                damage,
                wound_rolls,
                &mut dice.0,
            );
        }
        // A creature that the strike stuns and that lives through it leaves its swing or its
        // walk and stands, held for half a second (`0x56D70A`–`0x56D750`), the burning light's
        // sparks at its neck the while (816, `0x56D739`; #3081).
        if landed.stunned && health.current > 0 && victim.is_some() {
            debug!("fight: {} stuns {}", hit.attacker, hit.target);
            crate::holding_sights::burn(
                &mut commands,
                hit.target,
                opensacred_rules::particles::STUN_LIGHT,
            );
            let mut stunned = commands.entity(hit.target);
            stunned.remove::<(crate::npcs::Walk, crate::npcs::Strikes)>();
            stunned
                .entry::<crate::ai::Mind>()
                .and_modify(|mut mind| mind.shocked());
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
        struck.write(Struck {
            target: hit.target,
            damage,
            most: health.most,
        });
        // A hit that did damage makes its target speak, unless it falls (`0x532161`, #711), or
        // the hit is an object's (`0x5320D2`, #3471).
        if damage > 0 && health.current > 0 && !hit.object {
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
            // The killer's master where it is the hero's follower, hired or summoned (the hero's
            // list `+0x39C`, `0x4FC900`), whose kill drops (#2804) and makes the statistics'
            // record (below).
            let heros_follower = follower
                .and_then(|follower| attackers.get(follower.master).ok())
                .filter(|&(_, master_hero, ..)| master_hero && !hero_hit);
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
            // The hero's sets cut the rising chance where it kills (`0x532C6F`, #1727), and its
            // exorcism may lay the undead to rest (`0x532D9F`, M20).
            let sets = holding.filter(|_| hero).map(|holding| &holding.0.sets);
            let exorcism = (
                exorcist(holding, striker_attributes),
                rules.rare(opensacred_rules::undead::EXORCISM),
            );
            // The killing blow's whole damage and its fire's part as it came through, which
            // halves the chance by its roll (`0x532BFC`, #3385).
            let fire = whole(landed.through[1]);
            let rises = victim.and_then(|victim| {
                crate::undead::rolls(
                    &mut dice.0,
                    victim,
                    rank_of,
                    (plain, banished, undead_marked),
                    (damage, fire),
                    (sets, exorcism),
                )
                .inspect(|_| {
                    crate::undead::drain(
                        &mut commands,
                        (hit.attacker, hit.target),
                        (victim.creature, health.level),
                        level,
                    );
                })
            });
            // A creature that a raising stood up gives no experience (`0x534857`, #1856).
            let raised =
                victim.is_some_and(|victim| victim.flags & opensacred_rules::casts::RAISED != 0);
            // A creature's kill is its own, but a follower's of the hero, which goes whole to its
            // master (`0x534C2E`, #3394, below).
            if let Some(&worth) =
                worth.filter(|_| by_creature && heros_follower.is_none() && rises.is_none())
            {
                kills.write(Kill {
                    killer: hit.attacker,
                    victim: hit.target,
                    worth: if raised { Worth([0, 0]) } else { worth },
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
                heros_follower.map(|(combat, ..)| combat.0)
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
            // The items' and the blue potions' shares, and the hero's sets' (#1727, #2805). A
            // follower's kill gives its master the experience of the victim at the master's level,
            // with the master's items' effect 843 and the follower's own potions, of no sets
            // (`0x5347DB`–`0x53484C`), and the follower none (`0x534C2E`, #3394; docs/re/fight.md,
            // "Who is sent the experience").
            let master_holding =
                heros_follower.and_then(|(_, _, _, _, (_, _, holding, ..))| holding);
            let (holding_of, sets) = if hero {
                (
                    holding,
                    holding.map_or(0.0, |holding| holding.0.sets.experience()),
                )
            } else {
                (master_holding, 0.0)
            };
            // By the attributes of the hero, the killer or its follower's master.
            let attributes_of = if hero {
                striker_attributes
            } else {
                heros_follower.map_or([0.0; 6], |(combat, ..)| combat.0.attributes)
            };
            let bonus = items_experience(holding_of, attributes_of) + potions::mentored(&killer);
            // A kill of the horse's trample is the horse's: of the horse's level and of no
            // bonus of the hero's, the horse and its rider each taking it (docs/re/fight.md,
            // "The mount's and the rider's share", #3586).
            let (bonus, sets) = if mount_level.is_some() {
                (0.0, 0.0)
            } else {
                (bonus, sets)
            };
            let worth = worth
                .filter(|_| (hero || heros_follower.is_some()) && !raised)
                .map(|worth| (*worth, rank.copied(), (bonus, sets)));
            fell(
                &mut commands,
                (&mut dice, &mut experience, &rules),
                (hit.target, (by_creature, heros_follower.is_some()), rises),
                &health,
                (worth, mount_level),
            );
        }
    }
}

/// A creature as a strike's opponent, against whose class the other's items' slots count
/// (`opensacred_rules::fight::HurledAt`, #3417): the hero's side the heroes', a creature's its
/// mind's; with a ranged weapon in a hand or of the type's mark 8; riding.
fn opponent(
    is_hero: bool,
    mind: Option<&crate::ai::Mind>,
    (holds, arms, seat): (
        Option<&Holding>,
        Option<&crate::npcs::Arms>,
        Option<&crate::riding::Seat>,
    ),
) -> HurledAt {
    HurledAt {
        thrower: 0,
        side: if is_hero {
            opensacred_rules::minds::HEROES
        } else {
            mind.map_or(0, |mind| mind.side)
        },
        ranged: holds.is_some_and(|holds| holds.0.ranged)
            || arms.is_some_and(crate::npcs::Arms::ranged)
            || mind.is_some_and(|mind| mind.mark() & 0xFFF == MAGICIANS_MARK),
        mounted: seat.and_then(crate::riding::Seat::ridden).is_some()
            || mind.is_some_and(|mind| mind.rides.is_some()),
    }
}

/// The mark of the type's magicians (`+0x25F`'s low 12 bits), against whom the slots of the class
/// `0x80` count with those that hold a ranged weapon.
const MAGICIANS_MARK: u16 = 8;

/// The items whose slots a creature's record reads: the hero's worn, a creature's held and worn.
fn items_of(holds: Option<&Holding>, arms: Option<&crate::npcs::Arms>) -> Vec<ItemValues> {
    match (holds, arms) {
        (Some(holds), _) => holds.0.items.clone(),
        (None, Some(arms)) => arms.item_values(),
        (None, None) => Vec::new(),
    }
}

/// Kills each creature slain outright ([`Slay`]): its health 0, it dies at nobody's hands, no kill
/// and no experience (`npcs::Dies::unkilled`), with no strike, number or line; an immortal one
/// lives.
fn slay(
    mut commands: Commands,
    mut slays: MessageReader<Slay>,
    mut dice: ResMut<Dice>,
    mut targets: Query<(&mut Health, Has<Immortal>), Without<Dies>>,
) {
    for slay in slays.read() {
        let Ok((mut health, immortal)) = targets.get_mut(slay.target) else {
            continue;
        };
        if immortal || health.current == 0 {
            continue;
        }
        health.current = 0;
        info!("fight: {} is slain outright", slay.target);
        let mut dies = Dies::unkilled();
        dies.choice = dice.0.below(2);
        commands.entity(slay.target).insert(dies);
    }
}

/// Takes each strike from nobody ([`Burn`], `0x56B130` for the attacker's id 0;
/// `opensacred_rules::fight::nobodys_strike`): its damage, through the target's resistances, takes
/// its health down, shows over it and makes it speak, its poison poisons it, and at 0 it dies at
/// nobody's hands, no kill and no experience (`npcs::Dies::unkilled`); it makes no foe of anyone.
/// An immortal target takes none.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn burn(
    mut commands: Commands,
    mut burns: MessageReader<Burn>,
    mut dice: ResMut<Dice>,
    rules: Res<Rules>,
    mut targets: Query<
        (
            &mut Health,
            &Combat,
            &GlobalTransform,
            Has<Body>,
            Has<Immortal>,
            Option<&crate::ai::Mind>,
        ),
        Without<Dies>,
    >,
    mut struck: MessageWriter<Struck>,
) {
    for burn in burns.read() {
        let Ok((mut health, defender, at, hero_hit, immortal, mind)) = targets.get_mut(burn.target)
        else {
            continue;
        };
        if health.current == 0 {
            continue;
        }
        let landed = opensacred_rules::fight::nobodys_strike(
            defender.0,
            burn.damages,
            hero_hit,
            &rules,
            &mut dice.0,
        );
        // Shaddar's ward takes nobody's damage whole (`0x56DAD2`, #2919).
        let damage =
            if mind.is_some_and(|mind| opensacred_rules::shaddar::wards(mind.creature, false)) {
                0
            } else {
                landed.damage
            };
        if !immortal {
            health.current = health.current.saturating_sub(damage);
        }
        debug!(
            "fight: nobody strikes {} for {damage}, health {}/{}",
            burn.target, health.current, health.most
        );
        // Its poison, as any strike's (`0x56DFC8`): nobody's, by which the poisons it gives add
        // up as one source's, and whose strikes the poisoned one takes from itself.
        if let Some(poison) = poisons(&landed, &mut dice.0) {
            debug!(
                "fight: nobody poisons {}: {:.2} for {:.2} s",
                burn.target, poison.value, poison.seconds
            );
            commands
                .entity(burn.target)
                .entry::<crate::effects::Effects>()
                .or_default()
                .and_modify(move |mut effects| {
                    effects.poison(poison, Entity::PLACEHOLDER);
                });
        }
        struck.write(Struck {
            target: burn.target,
            damage,
            most: health.most,
        });
        // A strike from nobody is an FX object's, of the category 12: no line of the struck one
        // (`0x5320D2`, #3471).
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
            info!("fight: a fire burns {} to death", burn.target);
            let mut dies = Dies::unkilled();
            dies.choice = dice.0.below(2);
            commands.entity(burn.target).insert(dies);
        }
    }
}

/// Gives `target`, which lives through a strike of `attacker` of the whole damage `damage`, the
/// open wound that the rolls of `items`, `sets` and `wounded` give
/// (`opensacred_rules::fight::open_wound`; #1727, M15): the effect 23 of the value `⌊damage/10⌋`
/// from the attacker, unless it bleeds by then; and the blood fountains of the part 100 that its
/// rolls call (`blood_fountains::spurt_at_hit`, #3159).
#[expect(
    clippy::type_complexity,
    reason = "the rolls' inputs as `open_wound` takes them"
)]
fn wound(
    commands: &mut Commands,
    (target, attacker): (Entity, Entity),
    damage: u32,
    (items, sets, wounded): ((f32, bool), ([Option<f32>; 2], f32), bool),
    dice: &mut Rng,
) {
    let wounding = opensacred_rules::fight::open_wound(items, sets, wounded, dice);
    if wounding.fountains > 0 {
        crate::blood_fountains::spurt_at_hit(commands, target, wounding.fountains);
    }
    let Some(seconds) = wounding.seconds else {
        return;
    };
    let value = opensacred_rules::fight::wound_value(damage);
    debug!("fight: {attacker} wounds {target}: {value} for {seconds:.1} s");
    commands
        .entity(target)
        .entry::<crate::effects::Effects>()
        .or_default()
        .and_modify(move |mut effects| {
            effects.wound(value, seconds, attacker);
        });
}

/// The exorcism of a killer that holds `holding`, of the stored attributes `attributes`
/// (`0x532D39`): its items' effect 863 against an opponent of no class, grown by its sets
/// (`set_bonuses::Stats::exorcist`, #1727's M20); 0 for a creature, which carries no items.
fn exorcist(holding: Option<&Holding>, attributes: [f32; 6]) -> f32 {
    holding.map_or(0.0, |holding| {
        holding
            .0
            .sets
            .exorcist(opensacred_rules::fight::item_effect(
                &holding.0.items,
                opensacred_rules::undead::EXORCISM,
                attributes,
                Some(HurledAt::default()),
            ))
    })
}

/// A kind's damage as it came through, truncated (`__ftol`), none below 0.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a strike's damage, not negative, far below the integers' range"
)]
fn whole(damage: f32) -> u32 {
    damage.max(0.0) as u32
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

/// What a kill is worth to its earner: the victim's worth, its rank, and the earner's bonus and
/// its sets' share ([`fell`]).
type KillWorth = (Worth, Option<Ranked>, (f32, f32));

/// Fells `target`, at `health`: it dies, one of its two dying clips at random, and where the
/// hero killed it, of `worth`, its rank and the hero's bonuses (`potions::mentored`, its sets'),
/// the hero earns its experience (the event 9, `0x57D860`), twice for a boss and 1.5 times for a
/// champion; the experience earned. A creature
/// killed by another (`by_creature`, #630) drops nothing unless its killer is the hero's
/// follower (`by_follower`, #2804), and its killer earns its experience through the [`Kill`]
/// that [`resolve`] sends (`npcs::earn`, #690). An undead that `rises` gives nothing (`undead`,
/// #1199), and a skeleton warrior that rises falls as the body of the type it changes into
/// (`undead::change`, #1260). A kill of the horse that the hero rides (its trample's) is worked
/// out at the horse's level, `mount` (#3586).
fn fell(
    commands: &mut Commands,
    (dice, experience, rules): (&mut Dice, &mut Experience, &Rules),
    (target, (by_creature, by_follower), rises): (
        Entity,
        (bool, bool),
        Option<crate::undead::Rises>,
    ),
    health: &Health,
    (worth, mount): (Option<KillWorth>, Option<u32>),
) -> u32 {
    let choice = dice.0.below(2);
    let mut dies = if by_creature {
        Dies::by_creature(choice)
    } else {
        Dies::new(choice)
    };
    dies.by_follower = by_follower;
    dies.rises = rises.is_some();
    commands.entity(target).insert(dies);
    let Some(rises) = rises else {
        let earned = earn(experience, rules, (target, mount), health, worth);
        // Its mount takes the same, whole, where it rides (`0x5319B1`–`0x531A37`, #3393).
        if earned > 0 {
            commands.queue(move |world: &mut World| crate::npcs::mount_earns(world, earned));
        }
        return earned;
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
/// bonus ([`fell`]): of the victim's level and the hero's, or the level `mount` of the horse
/// whose kill it is.
fn earn(
    experience: &mut Experience,
    rules: &Rules,
    (target, mount): (Entity, Option<u32>),
    health: &Health,
    worth: Option<KillWorth>,
) -> u32 {
    let Some((worth, rank, bonus)) = worth else {
        return 0;
    };
    let killer = mount.unwrap_or(experience.level);
    let earned = experience_with(worth.0, health.level, killer, rules, bonus);
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
            Option<&Holding>,
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
            (Option<&Ranked>, &Combat, Has<crate::undead::Marked>),
        ),
        (Without<Dies>, Without<Body>),
    >,
) -> Result<Vec<String>, String> {
    let (at, effects, hero_combat, holding) = hero
        .iter()
        .next()
        .ok_or("the hero is not in the world yet")?;
    let at = at.translation;
    let effects = effects.map(|effects| effects.list()).unwrap_or_default();
    let slayer = Slayer::of(&effects, holding, hero_combat);
    let home = crate::paths::cell_of(at.x, at.z);
    let radius = i32::try_from(radius).unwrap_or(i32::MAX);
    let mut lines = Vec::new();
    for (entity, transform, mut health, mind, worth, (rank, combat, marked)) in &mut creatures {
        let here = transform.translation;
        let cell = crate::paths::cell_of(here.x, here.z);
        if !mind.hostile() || (cell.0 - home.0).abs() > radius || (cell.1 - home.1).abs() > radius {
            continue;
        }
        let earned = slayer.slay(
            &mut commands,
            (&mut dice, &mut experience, &rules),
            (tally.as_deref_mut(), records.as_deref_mut()),
            (entity, &mut health, mind, marked),
            (worth, rank, combat),
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

/// The share of a kill's base experience that the items of the hero of `holding`, of the stored
/// attributes `attributes`, earn it more, their effect 843
/// (`opensacred_rules::fight::items_experience`, #2805); none without them.
fn items_experience(holding: Option<&Holding>, attributes: [f32; 6]) -> f32 {
    holding.map_or(0.0, |holding| {
        opensacred_rules::fight::items_experience(&holding.0.items, attributes)
    })
}

/// The hero's state that a creature's fall to it reads ([`kill`], [`terrify`]): whether its
/// effects banish the undead, its items' and mentoring's and its sets' shares of the experience,
/// its sets and its items' exorcism, and its fight values for the statistics.
struct Slayer<'a> {
    banished: bool,
    bonus: (f32, f32),
    sets: Option<&'a opensacred_rules::set_bonuses::Stats>,
    holding: Option<&'a Holding>,
    combat: Option<&'a Combat>,
}

/// The stored attributes of a creature's `combat`, none without it.
fn attributes_of(combat: Option<&Combat>) -> [f32; 6] {
    combat.map_or([0.0; 6], |combat| combat.0.attributes)
}

impl<'a> Slayer<'a> {
    fn of(
        effects: &[opensacred_rules::fight::Effect],
        holding: Option<&'a Holding>,
        combat: Option<&'a Combat>,
    ) -> Self {
        let sets = holding.map(|holding| &holding.0.sets);
        Self {
            banished: potions::banishes(effects),
            bonus: (
                items_experience(holding, attributes_of(combat)) + potions::mentored(effects),
                sets.map_or(0.0, |sets| sets.experience()),
            ),
            sets,
            holding,
            combat,
        }
    }

    /// Fells `entity` as the hero's kill: its health 0, its rising rolled (`undead::rolls`), its
    /// experience earned but a raised one's (#1856), counted among the hero's kills (#1611) and in
    /// its statistics; the experience earned.
    fn slay(
        &self,
        commands: &mut Commands,
        (dice, experience, rules): (&mut Dice, &mut Experience, &Rules),
        (tally, records): (
            Option<&mut crate::statistics::Tally>,
            Option<&mut crate::statistics::Kills>,
        ),
        (entity, health, mind, marked): (Entity, &mut Health, &crate::ai::Mind, bool),
        (worth, rank, combat): (Option<&Worth>, Option<&Ranked>, &Combat),
    ) -> u32 {
        health.current = 0;
        let rises = crate::undead::rolls(
            &mut dice.0,
            mind,
            rank.map_or(Rank::Common, |rank| rank.0),
            (true, self.banished, marked),
            (0, 0),
            (
                self.sets,
                (
                    exorcist(self.holding, attributes_of(self.combat)),
                    rules.rare(opensacred_rules::undead::EXORCISM),
                ),
            ),
        );
        // A creature that a raising stood up gives no experience (#1856).
        let worth = worth
            .filter(|_| mind.flags & opensacred_rules::casts::RAISED == 0)
            .map(|worth| (*worth, rank.copied(), self.bonus));
        let earned = fell(
            commands,
            (dice, experience, rules),
            (entity, (false, false), rises),
            health,
            (worth, None),
        );
        if let Some(tally) = tally {
            tally.killed();
        }
        if let (Some(records), Some(hero)) = (records, self.combat) {
            let foe =
                crate::statistics::Foe::of(mind.creature, health, combat, rules.creatures_aw());
            let chances = (
                crate::statistics::chance(hero.attack, foe.defence),
                crate::statistics::chance(foe.attack, hero.defence),
            );
            records.record(foe, chances);
        }
        earned
    }
}

/// The terror of the hero's items (the effect 852, `opensacred_rules::fight::TERROR`): at each of
/// the hero's looks (`crowd::Nearest`, `0x547070`, every 0.3 s of the world's ticks), each
/// hostile creature that it lists and that its look terrifies
/// (`opensacred_rules::fight::terrifies`: none while the hero lies `Downed`, within 800, 400
/// while the hero walks, its level on
/// bronze's scale at most the threshold that the effect's value and the hero's level make, of no
/// big type, no boss nor champion, and no script holds it) falls at once as the hero's kill
/// (#1125), its weapon thrown down where `R(0, 100)` falls below the levels' difference
/// (`opensacred_rules::fight::terror_disarms`, `npcs::disarm`, #2298).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
pub fn terrify(
    mut commands: Commands,
    nearest: Res<crate::crowd::Nearest>,
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
            &Body,
            Option<&crate::effects::Effects>,
            Option<&Combat>,
            Option<&Holding>,
            Has<Downed>,
        ),
        Without<crate::ai::Mind>,
    >,
    mut creatures: Query<
        (
            &Transform,
            &mut Health,
            &crate::ai::Mind,
            Option<&Worth>,
            (Option<&Ranked>, &Combat, Has<crate::undead::Marked>),
            Has<crate::npcs::ScriptQueue>,
        ),
        (Without<Dies>, Without<Body>),
    >,
) {
    if !nearest.looked || nearest.hostiles.is_empty() {
        return;
    }
    let Some((at, body, effects, hero_combat, holding, downed)) = hero.iter().next() else {
        return;
    };
    let value = holding.map_or(0.0, |holding| {
        opensacred_rules::fight::item_effect(
            &holding.0.items,
            opensacred_rules::fight::TERROR,
            attributes_of(hero_combat),
            None,
        )
    });
    let threshold = rules.terror_threshold(value, experience.level);
    if threshold == 0 {
        return;
    }
    let effects = effects.map(|effects| effects.list()).unwrap_or_default();
    let slayer = Slayer::of(&effects, holding, hero_combat);
    let walks = body.moving().is_some();
    for &entity in &nearest.hostiles {
        let Ok((transform, mut health, mind, worth, (rank, combat, marked), queued)) =
            creatures.get_mut(entity)
        else {
            continue;
        };
        let distance = (transform.translation - at.translation).xz().length();
        let ranked = rank.is_some_and(|rank| rank.0 != Rank::Common);
        if mind.flags & opensacred_rules::ways::HOSTILE == 0
            || !opensacred_rules::fight::terrifies(
                threshold,
                rules.on_bronze(health.level),
                (distance, walks, downed),
                (mind.big, ranked, queued),
            )
        {
            continue;
        }
        info!(
            "fight: the hero's terror fells {entity}, a {} of level {}",
            mind.creature, health.level
        );
        let disarms = opensacred_rules::fight::terror_disarms(
            experience.level,
            health.level,
            dice.0.below(101),
        );
        slayer.slay(
            &mut commands,
            (&mut dice, &mut experience, &rules),
            (tally.as_deref_mut(), records.as_deref_mut()),
            (entity, &mut health, mind, marked),
            (worth, rank, combat),
        );
        if disarms {
            commands.queue(move |world: &mut World| crate::npcs::disarm(world, entity));
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

/// The regeneration (`0x562730`), on the world's tick as the creatures' updates run it
/// (`frame::WorldTick`, #616): each creature's value times its rate
/// (`Rules::regeneration_rate`), an NPC's `RegInFight`, the hero's `RegInFight` in the fight (a
/// hostile within 350, or 1,000 with a ranged weapon, its stance, #2914), `RegEnemyNear` with a
/// hostile within 1,000, else `RegNoEnemy`, times the tick's seconds, up to its most health; none
/// while it dies.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn regenerate(
    tick: Res<crate::frame::WorldTick>,
    rules: Res<Rules>,
    nearest: Res<crate::crowd::Nearest>,
    mut living: Query<(&mut Health, &mut Regenerates, Has<Body>, Option<&Holding>), Without<Dies>>,
) {
    let Some(seconds) = tick.seconds else {
        return;
    };
    let hostile = nearest.found.then_some(nearest.distance);
    for (mut health, mut regenerates, hero, holding) in &mut living {
        let ranged = holding.is_some_and(|holding| holding.0.ranged);
        let rate = rules.regeneration_rate(hero, hostile, ranged);
        let full = health.current == 0 || health.current >= health.most;
        let gained = regenerates.gather(rate, seconds, full);
        if gained > 0 {
            health.current = (health.current + gained).min(health.most);
        }
    }
}

/// The hero's level-up (`0x564D60` from `0x57E160`): its attack, defence, damage, resistances,
/// most health and regeneration by its new level, with what it holds and wears and its effects,
/// its pace and its arts' skills with them, as every other recomputation makes them (`0x5796A0`,
/// `statistics::refit`, #2821), its health refilled; a rise sounds `LEVELUP01` in the middle (`0x57E160` at `0x57EE84`, the event
/// `(9, 0x10)`; #1167) and bursts the hero into its gold sparks (`0x57F504`, `level_sparks`,
/// #2762).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
pub(crate) fn level_up(
    mut commands: Commands,
    experience: Res<Experience>,
    rules: Res<Rules>,
    mut heroes: Query<
        (
            Entity,
            &Base,
            Option<&Holding>,
            &mut Combat,
            &mut Health,
            &mut Regenerates,
            (
                Option<&crate::riding::Saddle>,
                Option<&crate::effects::Effects>,
            ),
        ),
        (With<Body>, Without<Dies>),
    >,
    mut sounds: MessageWriter<crate::sfx::Play>,
    (mut pace, mut arts): (
        Option<ResMut<crate::hero::Hero>>,
        Option<ResMut<crate::arts::HeroArts>>,
    ),
) {
    for (hero, base, holding, mut values, mut health, mut regenerates, (saddle, effects)) in
        &mut heroes
    {
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
            commands.entity(hero).insert(crate::level_sparks::Rises);
            // The followers it keeps rise with it (`0x564D60` at its end, #2896).
            commands.queue(move |world: &mut World| {
                crate::followers::rise_with(world, hero, level);
            });
        }
        let holds = holding.map(|holding| holding.0.clone()).unwrap_or_default();
        let skills = hero_skills(base, level, &holds);
        let list = effects
            .map(crate::effects::Effects::list)
            .unwrap_or_default();
        *values = Combat(opensacred_rules::fight::combat_with(
            base,
            level,
            holds.clone(),
            &skills,
            &list,
            &rules,
        ));
        let most = crate::riding::Saddle::raise(
            saddle,
            hero_health_held(base, level, &holds, &skills, &rules),
            level,
        );
        *health = Health {
            current: most,
            most,
            level,
        };
        regenerates.value =
            opensacred_rules::fight::regeneration_with(base, level, &skills, &list, &rules);
        if let Some(pace) = pace.as_deref_mut() {
            pace.pace = values.pace_factor();
        }
        if let Some(arts) = arts.as_deref_mut() {
            arts.skills = skills;
        }
    }
}

/// The hero's most health made anew as what it holds and wears changes (`0x5796A0`, #1365): its
/// items' and sets' bonuses to the attributes `a0` and `a3` and to its skill 10 raise it, and its
/// health stays, held at the most (docs/re/character.md: the stats made anew keep the current
/// health while the most rises).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn keep_health(
    experience: Res<Experience>,
    rules: Res<Rules>,
    mut heroes: Query<
        (&Base, &Holding, &mut Health, Option<&crate::riding::Saddle>),
        (With<Body>, Without<Dies>, Changed<Holding>),
    >,
) {
    for (base, holding, mut health, saddle) in &mut heroes {
        let level = experience.level;
        let skills = hero_skills(base, level, &holding.0);
        let most = crate::riding::Saddle::raise(
            saddle,
            hero_health_held(base, level, &holding.0, &skills, &rules),
            level,
        );
        if most != health.most {
            debug!(
                "fight: the hero's most health {} becomes {most}",
                health.most
            );
            health.most = most;
            health.current = health.current.min(most);
        }
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

/// What turned a spell object's strike or gift aside ([`turned_aside`]): the target's magic
/// defence, or its effect `0x1A` of the art `art` and the value `value`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Turned {
    Defence,
    Effect { art: u32, value: f32 },
}

/// The magic defence of a target of the stored attributes `attributes` (`0x561860`, the sets'
/// M17, #2277): the hero's items' 858 grown by its sets' terms (`guarded`, where `hero` the target
/// is the hero), else its arms' items'.
fn magic_defence(
    hero: bool,
    (guarded, arms): (Option<&Holding>, Option<&crate::npcs::Arms>),
    attributes: [f32; 6],
) -> f32 {
    match (hero, guarded) {
        (true, Some(guarded)) => {
            guarded
                .0
                .sets
                .magic_defence(opensacred_rules::fight::item_effect(
                    &guarded.0.items,
                    opensacred_rules::fight::MAGIC_DEFENCE,
                    attributes,
                    None,
                ))
        }
        _ => arms.map_or(0.0, |arms| {
            opensacred_rules::fight::item_effect(
                &arms.item_values(),
                opensacred_rules::fight::MAGIC_DEFENCE,
                attributes,
                None,
            )
        }),
    }
}

/// The magic lore of a spell's caster of the level `level`: the hero's skills' (`hero`), else its
/// arms' skill at its level.
fn caster_lore(
    (hero, arms, level): (bool, Option<&crate::npcs::Arms>, u32),
    hero_arts: Option<&crate::arts::HeroArts>,
    rules: &Rules,
) -> f32 {
    match (hero, arms) {
        (true, _) => hero_arts.map_or(0.0, |arts| {
            opensacred_rules::fight::magic_lore(&arts.skills, rules)
        }),
        (false, Some(arms)) => opensacred_rules::fight::magic_lore(
            &[(MAGIC_LORE, arms.skill_level(level, MAGIC_LORE))],
            rules,
        ),
        (false, None) => 0.0,
    }
}

/// Whether a spell object's strike or gift of a caster of the level `level` and the magic lore
/// `lore` is turned aside before it reaches its target (`0x561860`): the target's magic defence
/// `ward` by its roll (#2277), then its effect `0x1A` of `held` by the arrows' rule, `k` 1, by its
/// own (#2945), each of `dice`; `target` and `caster` for the log.
fn turned_aside(
    dice: &mut Dice,
    (ward, level, lore): (f32, u32, f32),
    held: Option<&crate::effects::Effects>,
    (target, caster): (Entity, Entity),
) -> Option<Turned> {
    if let Some(share) = opensacred_rules::fight::magic_ward(ward, level, lore) {
        let roll = dice.0.below(opensacred_rules::fight::MAGIC_DEFENCE_FACES);
        if opensacred_rules::fight::warded(share, roll) {
            debug!(
                "fight: {target} turns aside the spell of {caster} by its magic defence {ward} ({share:.3})"
            );
            return Some(Turned::Defence);
        }
    }
    let turns = held?
        .list()
        .into_iter()
        .find(|effect| effect.id == opensacred_rules::spells::TURNS_SPELLS)?;
    let chance = opensacred_rules::arrows::deflection_chance(turns.level, turns.art, level);
    let roll = f32::from(u16::try_from(dice.0.below(32_768)).unwrap_or(0)) / 32_767.0;
    (roll <= chance).then(|| {
        debug!(
            "fight: {target} turns aside the spell of {caster} by its effect of {} ({:.0} %)",
            turns.art,
            chance * 100.0
        );
        Turned::Effect {
            art: turns.art,
            value: turns.value,
        }
    })
}

/// Effects that a spell's object gives a target in place of a strike, which the target's magic
/// defence and its effect `0x1A` turn aside as they turn a strike (`0x561860` before
/// `0x5ABD80`'s event; the Daemon's fear 1012; [`gifts`], #3078).
#[derive(Message, Clone, Debug)]
pub struct Gift {
    pub caster: Entity,
    pub target: Entity,
    pub gives: Vec<(opensacred_rules::fight::Effect, f32)>,
}

/// Gives each living target of a [`Gift`] its effects unless the gift is turned aside
/// ([`turned_aside`]), a shield wall's flashing as for a strike.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn gifts(
    mut commands: Commands,
    mut gifts: MessageReader<Gift>,
    (mut dice, experience, rules, hero_arts): (
        ResMut<Dice>,
        Res<Experience>,
        Res<Rules>,
        Option<Res<crate::arts::HeroArts>>,
    ),
    effects: Query<&crate::effects::Effects>,
    casters: Query<(
        Has<Body>,
        Option<&crate::ai::Mind>,
        Option<&crate::npcs::Arms>,
    )>,
    targets: Query<
        (
            Has<Body>,
            Option<&Holding>,
            Option<&crate::npcs::Arms>,
            Option<&Combat>,
        ),
        Without<Dies>,
    >,
) {
    for gift in gifts.read() {
        let Ok((hero_hit, guarded, arms, combat)) = targets.get(gift.target) else {
            continue;
        };
        let (hero, mind, caster_arms) = casters.get(gift.caster).unwrap_or((false, None, None));
        let level = mind.map_or(experience.level, |mind| mind.level);
        let ward = magic_defence(hero_hit, (guarded, arms), attributes_of(combat));
        let lore = caster_lore((hero, caster_arms, level), hero_arts.as_deref(), &rules);
        let held = effects.get(gift.target).ok();
        match turned_aside(
            &mut dice,
            (ward, level, lore),
            held,
            (gift.target, gift.caster),
        ) {
            // The ward that turned it flashes (`0x562254`, #3172).
            Some(Turned::Effect { art, .. }) => {
                commands.write_message(crate::shields::Flash(gift.target, art));
            }
            Some(Turned::Defence) => {}
            None => crate::spells::give(&mut commands, gift.target, gift.gives.clone()),
        }
    }
}

/// The fight's rules.
pub struct FightPlugin;

impl Plugin for FightPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Hit>()
            .add_message::<Gift>()
            .add_message::<Burn>()
            .add_message::<Slay>()
            .add_message::<Kill>()
            .add_message::<Struck>()
            .insert_resource(Dice(crate::seed::dice(0)))
            .init_resource::<Experience>()
            .init_resource::<Rules>()
            .add_systems(
                OnEnter(AppState::InGame),
                |mut experience: ResMut<Experience>| *experience = Experience::default(),
            )
            .add_systems(
                Update,
                (resolve, gifts, burn, slay, level_up, keep_health, rise).chain(),
            )
            .add_systems(Update, calm.in_set(crate::frame::Unpaused))
            .add_systems(
                Update,
                terrify
                    .in_set(crate::frame::Unpaused)
                    .after(crate::frame::Step::Crowd)
                    .before(resolve),
            )
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
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Health {
    pub current: u32,
    pub most: u32,
    pub level: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hero_rising_a_level_keeps_its_effects_part_of_its_values() {
        let mut world = World::new();
        world.init_resource::<Messages<crate::sfx::Play>>();
        world.insert_resource(Rules::default());
        world.init_resource::<Experience>();
        world.resource_mut::<Experience>().level = 5;
        let base = opensacred_rules::fight::Base {
            attributes: [30, 20, 20, 25, 10, 10],
            ..opensacred_rules::fight::Base::default()
        };
        // The effect 1, `A`'s multiplier, half again.
        let mut effects = crate::effects::Effects::default();
        effects.add(opensacred_rules::fight::Effect::new(1, 0.5), 60.0);
        let rules = opensacred_rules::fight::Rules::default();
        let hero = world
            .spawn((
                Body::standing(),
                Base(base),
                Combat(opensacred_rules::fight::combat(
                    &base,
                    4,
                    Holds::default(),
                    &[],
                    &rules,
                )),
                Health {
                    current: 50,
                    most: 50,
                    level: 4,
                },
                Regenerates(opensacred_rules::fight::Regenerates::new(0.0)),
                effects.clone(),
            ))
            .id();
        world.run_system_cached(level_up).unwrap();
        world.flush();
        let skills = hero_skills(&base, 5, &Holds::default());
        let with = opensacred_rules::fight::combat_with(
            &base,
            5,
            Holds::default(),
            &skills,
            &effects.list(),
            &rules,
        );
        let without = combat(&base, 5, Holds::default(), &skills, &rules);
        let attack = world.get::<Combat>(hero).unwrap().0.attack;
        assert!(
            (attack - with.attack).abs() < 1e-4,
            "{attack} {}",
            with.attack
        );
        assert!(attack > without.attack);
        assert_eq!(world.get::<Health>(hero).unwrap().level, 5);
    }

    #[test]
    fn the_scripts_mark_spares_a_creature_without_a_hero_for_its_master() {
        // Marked, no hero its master: a creature's strike leaves it; a hero's clears the mark
        // from one of no dialogue nor function and harms it, but not from one of a dialogue or a
        // function.
        assert_eq!(
            marked_after(true, (false, false), (false, false, false)),
            (true, true)
        );
        assert_eq!(
            marked_after(true, (true, false), (false, false, false)),
            (false, false)
        );
        assert_eq!(
            marked_after(true, (true, false), (false, true, false)),
            (true, true)
        );
        assert_eq!(
            marked_after(true, (true, false), (false, false, true)),
            (true, true)
        );
        // A hero its master: the mark stays and spares nothing.
        assert_eq!(
            marked_after(true, (true, false), (true, false, false)),
            (true, false)
        );
        // Unmarked: harmed, but Sharifa takes the mark at every strike, a hero's too.
        assert_eq!(
            marked_after(false, (false, false), (false, false, false)),
            (false, false)
        );
        assert_eq!(
            marked_after(false, (true, true), (false, false, false)),
            (true, true)
        );
        assert_eq!(
            marked_after(true, (true, true), (false, false, false)),
            (true, true)
        );
        assert_eq!(
            marked_after(false, (false, true), (true, false, false)),
            (true, false)
        );
    }

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
            attack_factor: 1.0,
            defence_factor: 1.0,
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
            .add_message::<Struck>()
            .add_message::<crate::voices::Speak>()
            .add_message::<crate::sfx::Blow>()
            .add_message::<crate::remarks::Remark>()
            .add_message::<crate::drops::PileFromHit>()
            .add_message::<crate::sfx::Play>()
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
        // One that the scripts mark to rise again, of no hero for its master, which a creature's
        // strikes leave (#3294).
        let marked = world
            .spawn((
                combat,
                health,
                GlobalTransform::default(),
                crate::npcs::RisesAgain,
            ))
            .id();
        for target in [mortal, immortal, marked] {
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
                    ward: false,
                    hurled: false,
                    object: false,
                    mount: false,
                });
            }
        }
        app.update();
        let current = |entity| app.world().get::<Health>(entity).unwrap().current;
        assert!(current(mortal) < health.current);
        assert_eq!(current(immortal), health.current);
        assert_eq!(current(marked), health.current);
        assert!(app.world().get::<crate::npcs::RisesAgain>(marked).is_some());
    }

    #[test]
    fn an_objects_hit_plays_no_sound_at_its_target_and_calls_no_line() {
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
            attack_factor: 1.0,
            defence_factor: 1.0,
        });
        let health = Health {
            current: 1_000_000,
            most: 1_000_000,
            level: 1,
        };
        let heard = |object: bool| {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .add_message::<Hit>()
                .add_message::<Kill>()
                .add_message::<Struck>()
                .add_message::<crate::voices::Speak>()
                .add_message::<crate::sfx::Blow>()
                .add_message::<crate::remarks::Remark>()
                .add_message::<crate::drops::PileFromHit>()
                .add_message::<crate::sfx::Play>()
                .insert_resource(Dice(Rng::of(0, 0, 0)))
                .init_resource::<Experience>()
                .init_resource::<Rules>()
                .add_systems(Update, resolve);
            let world = app.world_mut();
            let caster = world.spawn((combat, GlobalTransform::default())).id();
            let target = world
                .spawn((combat, health, GlobalTransform::default()))
                .id();
            world.write_message(Hit {
                attacker: caster,
                target,
                share: 1.0,
                ranged: true,
                art: None,
                fixed: Some([40.0, 0.0, 0.0, 0.0]),
                sure: true,
                poison: false,
                ward: false,
                hurled: false,
                object,
                mount: false,
            });
            app.update();
            let world = app.world();
            (
                world.resource::<Messages<crate::sfx::Blow>>().len(),
                world.resource::<Messages<crate::voices::Speak>>().len(),
            )
        };
        assert_eq!(heard(false), (1, 1));
        assert_eq!(heard(true), (0, 0));
    }

    /// The attacker's health and the health that the struck one lost after one sure strike of
    /// 100 by a hero holding an item of the effect `effect` of the value `value`.
    fn after_a_strike_with(effect: u32, value: i16) -> (u32, u32) {
        after_a_hit_with(effect, value, false)
    }

    /// The attacker's health and what the struck one lost after a hit of 100 fixed damage, a
    /// poison's own strike where `poison`, by an attacker of an item of the effect `effect` of
    /// the value `value`.
    fn after_a_hit_with(effect: u32, value: i16, poison: bool) -> (u32, u32) {
        let combat = Combat(opensacred_rules::fight::Combat {
            attack: 200.0,
            defence: 10.0,
            damage: [[0.0; 2]; 4],
            resistance: [0.0; 4],
            damping: [0; 4],
            speed: 140.0,
            pace: 130.0,
            multipliers: [1.0; 4],
            resistance_factors: [1.0; 4],
            spells: [1.0; 4],
            cast: opensacred_rules::fight::CAST_SPEED,
            attributes: [0.0; 6],
            attack_factor: 1.0,
            defence_factor: 1.0,
        });
        let mut item = opensacred_rules::fight::ItemValues::default();
        item.bonuses[0] = opensacred_rules::fight::Bonus {
            condition: 0,
            effect,
            value,
        };
        let holding = Holding(opensacred_rules::fight::Holds {
            items: vec![item],
            ..Default::default()
        });
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<Hit>()
            .add_message::<Kill>()
            .add_message::<Struck>()
            .add_message::<crate::voices::Speak>()
            .add_message::<crate::sfx::Blow>()
            .add_message::<crate::remarks::Remark>()
            .add_message::<crate::drops::PileFromHit>()
            .add_message::<crate::sfx::Play>()
            .insert_resource(Dice(Rng::of(0, 0, 0)))
            .init_resource::<Experience>()
            .init_resource::<Rules>()
            .add_systems(Update, resolve);
        let world = app.world_mut();
        let health = |current| Health {
            current,
            most: 1_000,
            level: 1,
        };
        let hero = world
            .spawn((combat, health(50), holding, GlobalTransform::default()))
            .id();
        let target = world
            .spawn((combat, health(1_000), GlobalTransform::default()))
            .id();
        world.write_message(Hit {
            attacker: hero,
            target,
            share: 1.0,
            ranged: false,
            art: None,
            fixed: Some([100.0, 0.0, 0.0, 0.0]),
            sure: true,
            poison,
            ward: false,
            hurled: false,
            object: false,
            mount: false,
        });
        app.update();
        let current = |entity| app.world().get::<Health>(entity).unwrap().current;
        (current(hero), 1_000 - current(target))
    }

    /// What a creature lost to the hero's sure hit of 100 fixed damage, two hours into the game,
    /// a hurled item's where `hurled`.
    fn lost_two_hours_in(hurled: bool) -> u32 {
        let combat = Combat(opensacred_rules::fight::Combat {
            attack: 200.0,
            defence: 10.0,
            damage: [[0.0; 2]; 4],
            resistance: [0.0; 4],
            damping: [0; 4],
            speed: 140.0,
            pace: 130.0,
            multipliers: [1.0; 4],
            resistance_factors: [1.0; 4],
            spells: [1.0; 4],
            cast: opensacred_rules::fight::CAST_SPEED,
            attributes: [0.0; 6],
            attack_factor: 1.0,
            defence_factor: 1.0,
        });
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<Hit>()
            .add_message::<Kill>()
            .add_message::<Struck>()
            .add_message::<crate::voices::Speak>()
            .add_message::<crate::sfx::Blow>()
            .add_message::<crate::remarks::Remark>()
            .add_message::<crate::drops::PileFromHit>()
            .add_message::<crate::sfx::Play>()
            .insert_resource(Dice(Rng::of(0, 0, 0)))
            .insert_resource(crate::drops::Played::of(7_200.0))
            .init_resource::<Experience>()
            .init_resource::<Rules>()
            .add_systems(Update, resolve);
        let world = app.world_mut();
        let health = Health {
            current: 1_000,
            most: 1_000,
            level: 1,
        };
        let hero = world
            .spawn((combat, health, Body::standing(), GlobalTransform::default()))
            .id();
        let target = world
            .spawn((combat, health, GlobalTransform::default()))
            .id();
        world.write_message(Hit {
            attacker: hero,
            target,
            share: 1.0,
            ranged: false,
            art: None,
            fixed: Some([100.0, 0.0, 0.0, 0.0]),
            sure: true,
            poison: false,
            ward: false,
            hurled,
            object: false,
            mount: false,
        });
        app.update();
        1_000 - app.world().get::<Health>(target).unwrap().current
    }

    #[test]
    fn a_hurled_items_damages_take_no_survivals_raise() {
        // The hero's fixed damages rise by the survival's share of the time played (`0x56C156`);
        // a hurled item's take the attack record's place and none (`0x56BC81`, #3382).
        let raised = lost_two_hours_in(false);
        let hurled = lost_two_hours_in(true);
        assert_eq!(hurled, 100);
        assert!(raised > hurled, "{raised} {hurled}");
    }

    #[test]
    fn a_followers_kill_gives_its_experience_to_the_hero_and_none_to_the_follower() {
        let combat = Combat(opensacred_rules::fight::Combat {
            attack: 200.0,
            defence: 10.0,
            damage: [[0.0; 2]; 4],
            resistance: [0.0; 4],
            damping: [0; 4],
            speed: 140.0,
            pace: 130.0,
            multipliers: [1.0; 4],
            resistance_factors: [1.0; 4],
            spells: [1.0; 4],
            cast: opensacred_rules::fight::CAST_SPEED,
            attributes: [0.0; 6],
            attack_factor: 1.0,
            defence_factor: 1.0,
        });
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<Hit>()
            .add_message::<Kill>()
            .add_message::<Struck>()
            .add_message::<crate::voices::Speak>()
            .add_message::<crate::sfx::Blow>()
            .add_message::<crate::remarks::Remark>()
            .add_message::<crate::drops::PileFromHit>()
            .add_message::<crate::sfx::Play>()
            .insert_resource(Dice(Rng::of(0, 0, 0)))
            .init_resource::<Experience>()
            .init_resource::<Rules>()
            .add_systems(Update, resolve);
        let world = app.world_mut();
        let health = |current| Health {
            current,
            most: 1_000,
            level: 5,
        };
        let hero = world
            .spawn((
                combat,
                health(500),
                Body::standing(),
                GlobalTransform::default(),
            ))
            .id();
        let follower = world
            .spawn((
                combat,
                health(500),
                crate::followers::Follower::of(hero),
                GlobalTransform::default(),
            ))
            .id();
        let victim = world
            .spawn((
                combat,
                health(10),
                Worth([100, 0]),
                GlobalTransform::default(),
            ))
            .id();
        let before = world.resource::<Experience>().points;
        world.write_message(Hit {
            attacker: follower,
            target: victim,
            share: 1.0,
            ranged: false,
            art: None,
            fixed: Some([100.0, 0.0, 0.0, 0.0]),
            sure: true,
            poison: false,
            ward: false,
            hurled: false,
            object: false,
            mount: false,
        });
        app.update();
        assert_eq!(app.world().get::<Health>(victim).unwrap().current, 0);
        assert!(app.world().resource::<Experience>().points > before);
        assert!(app.world().resource::<Messages<Kill>>().is_empty());
    }

    #[test]
    fn a_trample_kill_is_worked_out_at_the_horses_level() {
        let combat = Combat(opensacred_rules::fight::Combat {
            attack: 200.0,
            defence: 10.0,
            damage: [[0.0; 2]; 4],
            resistance: [0.0; 4],
            damping: [0; 4],
            speed: 140.0,
            pace: 130.0,
            multipliers: [1.0; 4],
            resistance_factors: [1.0; 4],
            spells: [1.0; 4],
            cast: opensacred_rules::fight::CAST_SPEED,
            attributes: [0.0; 6],
            attack_factor: 1.0,
            defence_factor: 1.0,
        });
        // The hero's earning of a kill by its strike of the horse's trample or its own.
        let earned = |mount: bool| {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .add_message::<Hit>()
                .add_message::<Kill>()
                .add_message::<Struck>()
                .add_message::<crate::voices::Speak>()
                .add_message::<crate::sfx::Blow>()
                .add_message::<crate::remarks::Remark>()
                .add_message::<crate::drops::PileFromHit>()
                .add_message::<crate::sfx::Play>()
                .insert_resource(Dice(Rng::of(0, 0, 0)))
                .init_resource::<Experience>()
                .init_resource::<Rules>()
                .add_systems(Update, resolve);
            let world = app.world_mut();
            let health = |current, level| Health {
                current,
                most: 1_000,
                level,
            };
            let horse = world
                .spawn((combat, health(500, 30), GlobalTransform::default()))
                .id();
            let hero = world
                .spawn((
                    combat,
                    health(500, 1),
                    Body::standing(),
                    crate::riding::Seat::Riding {
                        horse,
                        speeds: [1.0; 2],
                        offset: Vec3::ZERO,
                    },
                    GlobalTransform::default(),
                ))
                .id();
            let victim = world
                .spawn((
                    combat,
                    health(10, 1),
                    Worth([100, 10]),
                    GlobalTransform::default(),
                ))
                .id();
            let before = world.resource::<Experience>().points;
            world.write_message(Hit {
                attacker: hero,
                target: victim,
                share: 1.0,
                ranged: false,
                art: None,
                fixed: Some([100.0, 0.0, 0.0, 0.0]),
                sure: true,
                poison: false,
                ward: false,
                hurled: false,
                object: false,
                mount,
            });
            app.update();
            assert_eq!(app.world().get::<Health>(victim).unwrap().current, 0);
            app.world().resource::<Experience>().points - before
        };
        let rules = Rules::default();
        let hero_level = Experience::default().level;
        let at = |level| u64::from(experience_with([100, 10], 1, level, &rules, (0.0, 0.0)));
        assert_eq!(earned(true), at(30));
        assert_eq!(earned(false), at(hero_level));
        assert_ne!(at(30), at(hero_level));
    }

    /// What a creature of a poison's resistance of 50 lost to a strike of 8 poison, a poison's own
    /// strike where `poison`.
    fn lost_to_eight_poison(poison: bool) -> u32 {
        let combat = |resistance| {
            Combat(opensacred_rules::fight::Combat {
                attack: 200.0,
                defence: 10.0,
                damage: [[0.0; 2]; 4],
                resistance,
                damping: [0; 4],
                speed: 140.0,
                pace: 130.0,
                multipliers: [1.0; 4],
                resistance_factors: [1.0; 4],
                spells: [1.0; 4],
                cast: opensacred_rules::fight::CAST_SPEED,
                attributes: [0.0; 6],
                attack_factor: 1.0,
                defence_factor: 1.0,
            })
        };
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<Hit>()
            .add_message::<Kill>()
            .add_message::<Struck>()
            .add_message::<crate::voices::Speak>()
            .add_message::<crate::sfx::Blow>()
            .add_message::<crate::remarks::Remark>()
            .add_message::<crate::drops::PileFromHit>()
            .add_message::<crate::sfx::Play>()
            .insert_resource(Dice(Rng::of(0, 0, 0)))
            .init_resource::<Experience>()
            .init_resource::<Rules>()
            .add_systems(Update, resolve);
        let world = app.world_mut();
        let health = Health {
            current: 1_000,
            most: 1_000,
            level: 1,
        };
        let poisoner = world
            .spawn((combat([0.0; 4]), health, GlobalTransform::default()))
            .id();
        let target = world
            .spawn((
                combat([0.0, 0.0, 0.0, 50.0]),
                health,
                GlobalTransform::default(),
            ))
            .id();
        world.write_message(Hit {
            attacker: poisoner,
            target,
            share: 1.0,
            ranged: false,
            art: None,
            fixed: Some([0.0, 0.0, 0.0, 8.0]),
            sure: true,
            poison,
            ward: false,
            hurled: false,
            object: false,
            mount: false,
        });
        app.update();
        1_000 - app.world().get::<Health>(target).unwrap().current
    }

    #[test]
    fn a_poisons_own_strike_meets_no_resistance() {
        // No record of the struck one: its 8 whole (`0x56B6AA`, #3482); a strike of fixed poison
        // that is no poison's own meets the resistance of 50.
        assert_eq!(lost_to_eight_poison(true), 8);
        assert!(lost_to_eight_poison(false) < 8);
    }

    /// What a goblin warrior lost to the hero's sure strike of 20 to 20 physical, the hero holding
    /// an item of an 801 of 10 against the side `side`.
    fn lost_to_a_slot_against(side: u8) -> u32 {
        let combat = Combat(opensacred_rules::fight::Combat {
            attack: 200.0,
            defence: 10.0,
            damage: [[20.0, 20.0], [0.0; 2], [0.0; 2], [0.0; 2]],
            resistance: [0.0; 4],
            damping: [0; 4],
            speed: 140.0,
            pace: 130.0,
            multipliers: [1.0; 4],
            resistance_factors: [1.0; 4],
            spells: [1.0; 4],
            cast: opensacred_rules::fight::CAST_SPEED,
            attributes: [0.0; 6],
            attack_factor: 1.0,
            defence_factor: 1.0,
        });
        let mut item = opensacred_rules::fight::ItemValues::default();
        item.bonuses[0] = opensacred_rules::fight::Bonus {
            condition: u16::from(side),
            effect: 801,
            value: 10,
        };
        let holding = Holding(opensacred_rules::fight::Holds {
            items: vec![item],
            ..Default::default()
        });
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<Hit>()
            .add_message::<Kill>()
            .add_message::<Struck>()
            .add_message::<crate::voices::Speak>()
            .add_message::<crate::sfx::Blow>()
            .add_message::<crate::remarks::Remark>()
            .add_message::<crate::drops::PileFromHit>()
            .add_message::<crate::sfx::Play>()
            .insert_resource(Dice(Rng::of(0, 0, 0)))
            .init_resource::<Experience>()
            .init_resource::<Rules>()
            .add_systems(Update, resolve);
        let world = app.world_mut();
        let health = Health {
            current: 1_000,
            most: 1_000,
            level: 1,
        };
        let hero = world
            .spawn((
                combat,
                health,
                holding,
                Body::standing(),
                GlobalTransform::default(),
            ))
            .id();
        let mind = crate::ai::Mind::new(
            Vec3::ZERO,
            0x1,
            (false, false),
            None,
            [70.0, 180.0],
            (76, 8),
        );
        assert_eq!(mind.side, 8);
        let goblin = world
            .spawn((combat, health, mind, GlobalTransform::default()))
            .id();
        world.write_message(Hit {
            attacker: hero,
            target: goblin,
            share: 1.0,
            ranged: false,
            art: None,
            fixed: None,
            sure: true,
            poison: false,
            ward: false,
            hurled: false,
            object: false,
            mount: false,
        });
        app.update();
        1_000 - app.world().get::<Health>(goblin).unwrap().current
    }

    #[test]
    fn the_heros_slot_against_a_class_raises_its_strike_at_one_of_it() {
        // 20 against the side 8 is 30 (#3417); against another side, 20.
        let of_it = lost_to_a_slot_against(8);
        let other = lost_to_a_slot_against(5);
        assert_eq!(other, 20);
        assert!(of_it > other, "{of_it} {other}");
    }

    #[test]
    fn a_poisons_strike_pays_for_its_poisoners_842_and_leeches_nothing() {
        // As the original's, every strike of the poison that the wearer's weapon left reaches
        // 842's block (seen, #3081, #3373), and none its 841 (`0x5708D3`).
        let (paid, _) = after_a_hit_with(opensacred_rules::fight::PAINKILLER, 20, true);
        assert!((1..40).contains(&paid), "{paid}");
        let (leeched, _) = after_a_hit_with(opensacred_rules::fight::LEECH, 50, true);
        assert_eq!(leeched, 50);
    }

    #[test]
    fn a_strike_pays_for_its_842_and_leeches_by_its_841() {
        // Half its damage taken back at a distance of 0; a fifth of it paid, less 1, which the
        // struck one does not lose, its health fallen before the raise.
        let (plain, lost) = after_a_strike_with(opensacred_rules::fight::CRITICAL, 0);
        assert_eq!(plain, 50);
        let (leeched, _) = after_a_strike_with(opensacred_rules::fight::LEECH, 50);
        assert!(leeched > 80, "{leeched}");
        let (paid, lost_too) = after_a_strike_with(opensacred_rules::fight::PAINKILLER, 20);
        assert!((1..40).contains(&paid), "{paid}");
        assert_eq!(lost_too, lost);
    }

    #[test]
    fn a_gift_of_a_spells_object_is_turned_aside_as_its_strike_would_be() {
        // The fear's effect at a creature of no magic defence comes always; at one of a great
        // magic defence, as a strike, mostly not (#3078).
        let mut dice = Dice(crate::seed::dice(0));
        let (target, caster) = (Entity::from_raw_u32(1).expect("an id"), Entity::PLACEHOLDER);
        let turned = |dice: &mut Dice, ward: f32| {
            (0..100)
                .filter(|_| turned_aside(dice, (ward, 10, 0.0), None, (target, caster)).is_some())
                .count()
        };
        assert_eq!(turned(&mut dice, 0.0), 0);
        let warded = turned(&mut dice, 1_000.0);
        assert!(warded > 80, "{warded}");
        assert_eq!(
            turned_aside(
                &mut Dice(crate::seed::dice(0)),
                (1e9, 1, 0.0),
                None,
                (target, caster)
            ),
            Some(Turned::Defence)
        );
    }

    /// The hero's health after ten spell strikes of 100 at it by a creature of level 1, each
    /// asking its magic defence where `ward`, the hero's items' 858 of `value`.
    fn after_ten_spells(ward: bool, value: i16) -> u32 {
        let combat = Combat(opensacred_rules::fight::Combat {
            attack: 200.0,
            defence: 10.0,
            damage: [[0.0; 2]; 4],
            resistance: [0.0; 4],
            damping: [0; 4],
            speed: 140.0,
            pace: 130.0,
            multipliers: [1.0; 4],
            resistance_factors: [1.0; 4],
            spells: [1.0; 4],
            cast: opensacred_rules::fight::CAST_SPEED,
            attributes: [0.0; 6],
            attack_factor: 1.0,
            defence_factor: 1.0,
        });
        let mut item = opensacred_rules::fight::ItemValues::default();
        item.bonuses[0] = opensacred_rules::fight::Bonus {
            condition: 0,
            effect: opensacred_rules::fight::MAGIC_DEFENCE,
            value,
        };
        let holding = Holding(opensacred_rules::fight::Holds {
            items: vec![item],
            ..Default::default()
        });
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<Hit>()
            .add_message::<Kill>()
            .add_message::<Struck>()
            .add_message::<crate::voices::Speak>()
            .add_message::<crate::sfx::Blow>()
            .add_message::<crate::remarks::Remark>()
            .add_message::<crate::drops::PileFromHit>()
            .add_message::<crate::sfx::Play>()
            .insert_resource(Dice(Rng::of(0, 0, 0)))
            .init_resource::<Experience>()
            .init_resource::<Rules>()
            .add_systems(Update, resolve);
        let world = app.world_mut();
        let health = Health {
            current: 1_000,
            most: 1_000,
            level: 1,
        };
        let hero = world
            .spawn((
                combat,
                health,
                holding,
                crate::hero::Body::standing(),
                GlobalTransform::default(),
            ))
            .id();
        let caster = world
            .spawn((combat, health, GlobalTransform::default()))
            .id();
        for _ in 0..10 {
            world.write_message(Hit {
                attacker: caster,
                target: hero,
                share: 1.0,
                ranged: false,
                art: None,
                fixed: Some([100.0, 0.0, 0.0, 0.0]),
                sure: true,
                poison: false,
                ward,
                hurled: false,
                object: false,
                mount: false,
            });
        }
        app.update();
        app.world().get::<Health>(hero).unwrap().current
    }

    #[test]
    fn the_heros_terror_fells_the_weak_hostiles_at_its_look() {
        let combat = Combat(opensacred_rules::fight::Combat {
            attack: 10.0,
            defence: 10.0,
            damage: [[0.0; 2]; 4],
            resistance: [0.0; 4],
            damping: [0; 4],
            speed: 140.0,
            pace: 130.0,
            multipliers: [1.0; 4],
            resistance_factors: [1.0; 4],
            spells: [1.0; 4],
            cast: opensacred_rules::fight::CAST_SPEED,
            attributes: [0.0; 6],
            attack_factor: 1.0,
            defence_factor: 1.0,
        });
        let mut item = opensacred_rules::fight::ItemValues::default();
        item.bonuses[0] = opensacred_rules::fight::Bonus {
            condition: 0,
            effect: opensacred_rules::fight::TERROR,
            value: 10,
        };
        let holding = Holding(opensacred_rules::fight::Holds {
            items: vec![item],
            ..Default::default()
        });
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(Dice(Rng::of(0, 0, 0)))
            .init_resource::<Experience>()
            .init_resource::<Rules>()
            .init_resource::<crate::crowd::Nearest>()
            .add_systems(Update, terrify);
        app.world_mut().resource_mut::<Experience>().level = 20;
        let world = app.world_mut();
        world.spawn((
            Transform::default(),
            crate::hero::Body::standing(),
            combat,
            holding,
        ));
        // A goblin of level 7 and one of level 8, 300 off: the hero of level 20 with 10 of the
        // terror on bronze fells those of a level up to 7 (`Rules::terror_threshold`).
        let goblin = |world: &mut World, level| {
            let mind = crate::ai::Mind::new(
                Vec3::ZERO,
                opensacred_rules::ways::HOSTILE,
                (false, false),
                None,
                [70.0, 180.0],
                (76, 1),
            );
            let health = Health {
                current: 50,
                most: 50,
                level,
            };
            world
                .spawn((Transform::from_xyz(300.0, 0.0, 0.0), health, mind, combat))
                .id()
        };
        let weak = goblin(world, 7);
        let strong = goblin(world, 8);
        let mut nearest = world.resource_mut::<crate::crowd::Nearest>();
        nearest.looked = true;
        nearest.hostiles = vec![weak, strong];
        app.update();
        let fell = |entity| app.world().get::<Dies>(entity).is_some();
        assert!(fell(weak) && !fell(strong));
        assert_eq!(app.world().get::<Health>(weak).unwrap().current, 0);
    }

    #[test]
    fn the_magic_defence_turns_a_spells_strike_aside_whole() {
        // Ten strikes that ask none land, and so do those at a hero without the effect.
        let plain = after_ten_spells(false, 1_000);
        assert!(plain < 1_000);
        assert_eq!(after_ten_spells(true, 0), plain);
        // 858 of 1000 against a caster of level 1: 99 rolls of 100 turn one aside, at most a
        // strike or two landing.
        let lost = (1_000 - plain) / 10;
        assert!(after_ten_spells(true, 1_000) >= 1_000 - 2 * lost);
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
