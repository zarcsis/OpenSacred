//! The creatures' life and their way of fighting (docs/re/ai.md): each is dormant until its own
//! timer or the hero's look wakes it, and dormant again farther than 1,500 from the hero; a
//! hostile creature in the hero's reach steps about by the idle action (`0x54EA30`), a group's
//! members together; a peaceful animal awake flees, steps away and strolls about home, and a
//! guard keeps its post, and a townsperson rolls what it does next at each of the hero's looks,
//! by their behaviours (`0x4EF150`); a creature that shares its cell with another parts from it,
//! and one that walks into the hero's way steps aside. An awake hostile creature that the test of
//! the levels sends at the hero, or that has the hero within its reach, comes at it from up to
//! 800 along a straight line of walkable cells and keeps the band of its reach, where it strikes;
//! wounded it flees, and too far from home it goes back ([`pursue`],
//! docs/decisions/fight.md, "Pursuit's chain and wait"); the second to the fourth of the
//! hero's hostiles beside the nearest take their places about the hero for a tick; one with a
//! ranged weapon keeps 300 to 800 from the hero and shoots, goes to melee when the hero attacks it
//! from near and takes a bow again beyond its reach (`npcs::Rearms`, docs/decisions/fight.md,
//! "Archers' melee switch and rearming"); one that the test holds back goes dormant again and steps
//! about as one beyond it.
//! Without a straight line or a way to the hero, a creature goes for the nearest closed door of
//! its sector that a hero turned, and opens it ([`open`], docs/decisions/fight.md, "Doors' measured
//! points and giving up").
//! The creatures of hostile sides fight one another (#630): a guard awake attacks the nearest
//! creature hostile to it within 500, and an idle hostile creature, in one tick of three, takes
//! for its foe instead of the hero a creature that fights near it; they fight as they fight the
//! hero, in melee or at range (#670; docs/re/ai.md, "Foes among the creatures").
//! By night the guards and the townsfolk take torches into their left hands, and put them away by
//! day (#61; docs/re/ai.md, "In `opensacred-game`").
//! A caster decides its cast before its pursuit (`minds::Mind::cast`, #1852): `creature_casts`
//! casts its spell, `creature_arts` makes its art, its leap, its daggers and its stare, and its
//! blink moves it here.
//! A struck creature raises its group's alarm ([`alarm`], #669), a creature that first sees the
//! hero runs its sight function ([`sight`], #2475), and one of a dialogue that stands turns to
//! the hero ([`face`], #2543).
//! The decisions are the rules' (`opensacred_rules::minds`); this module keeps the minds as
//! components, reads the world for them and carries them out: the walks, the strikes, the shots,
//! the doors, the lines and the turns. They decide at each of the world's ticks alone
//! (`frame::WorldTick`, `0x5375C0`, #617), as the original's creatures' updates run, while their
//! walks and clips go on at every frame.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
pub use opensacred_rules::minds::of_type;
use opensacred_rules::minds::{
    self, AT_THE_DOOR, BEHAVES_WITHIN, Blocked, Child, Decision, DoorSeen, Evening, Foe, Pursues,
    Pursuit, Senses, Spot, Walking, Why, activity, child, daemon_fear, daemon_near,
    door_in_the_way, evening_way, follow, opens, runs_from, steps_off, townsperson, trader_for,
};
use opensacred_rules::ways::DoorPoints;

use crate::buildings::Layered;
use crate::crowd::{Crowd, InTheWay, Nearest};
use crate::daynight::{Clock, Phase};
use crate::frame::Step;
use crate::hero::Body;
use crate::lines::Says;
use crate::npcs::{Door, Npcs, Object, Plays, PutAt, Rearms, Strikes, Swing, Talks, Walk};
use crate::paths;
use crate::spawning::Rng;
use crate::terrain::{Heights, Level};

/// How near a door's point, along each axis, a creature going for the door stands where it is
/// (`0x53A170`).
const AT_THE_DOOR_POINT: f32 = 25.0;
/// How near a trader a townsperson that visits it stops (`0x4EE660`, `+0x176`).
const AT_THE_TRADER: f32 = 150.0;
/// How near a child at play the other idle children follow it (`0x4F2AD7`, `0x4EE820`).
const PLAYMATES: f32 = 600.0;
/// The cells' flag of a settlement's ground (`+0x1E` bit 1, docs/02 §5.2).
const SETTLEMENT: u8 = 0x2;
/// How near a hostile of the hero's puts a townsperson in danger (`0x4F0BDA`).
const DANGER: f32 = 800.0;
/// How far a townsperson seeks a door, and the cells about a door it goes in to and out to, in
/// how many tries (`0x4EE300`, `0x4EEFA0`, `0x4F1AB9`).
const DOOR_WITHIN: f32 = 1_500.0;
const INDOORS: (i32, i32) = (2, 9);
const OUTDOORS: (i32, i32) = (-4, 5);
const DOOR_TRIES: usize = 15;
/// The shade of the children's lines, green (**hypothesis**), and of the townsfolk's fear of the
/// Daemon, red (`0xFFFF3030`).
const CHILD_GREEN: Color = Color::srgb(0.19, 1.0, 0.19);
const FEAR_RED: Color = Color::srgb(1.0, 0.19, 0.19);

/// Whether the hero is the Daemon, its type 9, before whom the townsfolk are in danger and the
/// children flee (docs/re/ai.md, "Danger and the Daemon").
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct Daemon(pub bool);

/// A creature's mind (`opensacred_rules::minds::Mind`), and the creatures it goes to.
#[derive(Component, Clone, Debug, Deref, DerefMut)]
pub struct Mind {
    /// Its home, its flags word, its kind and speeds, its level, and its thought and its fight.
    #[deref]
    pub thinks: minds::Mind,
    /// The creature it went to last, which it does not go to next (`+0x24D`).
    last: Option<Entity>,
    /// The partner it follows (`+0x251`, the flag 4), and whether it runs after it.
    partner: Option<Entity>,
    runs_after: bool,
    /// The trader it walks to, at which it talks.
    visiting: Option<Entity>,
    /// The door it goes through, which it closes behind it (`+0x124`).
    door: Option<Entity>,
    /// The door in its way that it goes to open (the AI's state 1, `0x53A170`).
    opening: Option<Entity>,
    /// The door it goes into a house by, from its outer point, where the door stands, and the
    /// inner point it walks to once it opens it (the sub-states 8, 9 and 7, #431).
    entering: Option<(Entity, Vec3, Vec3)>,
    /// The creature it fights, its target (`+0x104`), while it fights one rather than the hero
    /// (#630).
    foe: Option<Entity>,
    /// Its script's spell (`+0x2B6`, its `CreateNPC`'s `0x67`), which a magician's chooser takes
    /// first (`0x4D31C0`, #1869); 0 for none.
    pub spell: u8,
    /// Whether its next strike in melee chains on the one that ended (`Mind::ends_strike`, #466).
    chains: bool,
}

/// A creature's mind as a save keeps it (#3757; the original's creature's AI's state and order,
/// `+0xFC` to `+0x14F`, docs/re/saves.md, "A creature's record"): its thought whole, its clock of
/// the real time from the save's time ([`minds::Mind::rebase`]), and the creatures and doors it
/// goes to by their names (`names`), none where a load finds none.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedMind {
    thinks: minds::Mind,
    #[serde(with = "crate::names::optional")]
    last: Option<Entity>,
    #[serde(with = "crate::names::optional")]
    partner: Option<Entity>,
    runs_after: bool,
    #[serde(with = "crate::names::optional")]
    visiting: Option<Entity>,
    #[serde(with = "crate::names::optional")]
    door: Option<Entity>,
    #[serde(with = "crate::names::optional")]
    opening: Option<Entity>,
    entering: Option<SavedEntering>,
    #[serde(with = "crate::names::optional")]
    foe: Option<Entity>,
    spell: u8,
    chains: bool,
}

/// The door a creature goes into a house by, as a save keeps it, and its outer and inner points.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct SavedEntering {
    #[serde(with = "crate::names::required")]
    door: Entity,
    outer: Vec3,
    inner: Vec3,
}

impl bevy::ecs::entity::MapEntities for SavedMind {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        for entity in [
            &mut self.last,
            &mut self.partner,
            &mut self.visiting,
            &mut self.door,
            &mut self.opening,
            &mut self.foe,
        ]
        .into_iter()
        .flatten()
        {
            *entity = mapper.get_mapped(*entity);
        }
        if let Some(entering) = &mut self.entering {
            entering.door = mapper.get_mapped(entering.door);
        }
    }
}

impl crate::doings::Saves for Mind {
    type Saved = SavedMind;

    fn saved(&self, at: &crate::doings::Moment) -> Option<SavedMind> {
        let mut thinks = self.thinks.clone();
        thinks.rebase(-at.now);
        Some(SavedMind {
            thinks,
            last: self.last,
            partner: self.partner,
            runs_after: self.runs_after,
            visiting: self.visiting,
            door: self.door,
            opening: self.opening,
            entering: self.entering.map(|(door, outer, inner)| SavedEntering {
                door,
                outer,
                inner,
            }),
            foe: self.foe,
            spell: self.spell,
            chains: self.chains,
        })
    }

    fn again(saved: SavedMind, at: &crate::doings::Moment) -> Option<Self> {
        use crate::names::found;
        let mut thinks = saved.thinks;
        thinks.rebase(at.now);
        Some(Self {
            thinks,
            last: found(saved.last),
            partner: found(saved.partner),
            runs_after: saved.runs_after,
            visiting: found(saved.visiting),
            door: found(saved.door),
            opening: found(saved.opening),
            entering: saved
                .entering
                .filter(|entering| entering.door != Entity::PLACEHOLDER)
                .map(|entering| (entering.door, entering.outer, entering.inner)),
            foe: found(saved.foe),
            spell: saved.spell,
            chains: saved.chains,
        })
    }
}

impl Mind {
    /// The mind of a creature made at `home` with the flags word `flags`, by an `Enter` function
    /// or not, by `CreateNPC` or a fill (`opensacred_rules::minds::Mind::new`).
    pub fn new(
        home: Vec3,
        flags: u32,
        made: (bool, bool),
        group: Option<u32>,
        speeds: [f32; 2],
        kind: (u32, u8),
    ) -> Self {
        Self {
            thinks: minds::Mind::new(spot(home), flags, made, group, speeds, kind),
            last: None,
            partner: None,
            runs_after: false,
            visiting: None,
            door: None,
            opening: None,
            entering: None,
            foe: None,
            spell: 0,
            chains: false,
        }
    }

    /// The creature it fights, while it fights one rather than the hero (#630).
    pub fn foe(&self) -> Option<Entity> {
        self.foe.filter(|_| self.fighting())
    }

    /// A follower called off its fight, its foe forgotten, to be put near its master
    /// (`followers`).
    pub fn called_off(&mut self) {
        self.thinks.loses_foe();
        self.foe = None;
    }

    /// A follower sent at `foe` (`minds::Mind::sent_at`, the event `0x100`); whether it goes.
    pub fn send_at(&mut self, foe: Entity) -> bool {
        let goes = self.thinks.sent_at();
        if goes {
            self.foe = Some(foe);
        }
        goes
    }

    /// A creature struck turns on its attacker, the hero where `attacker` is none, unless it is
    /// held or `flying` in a throw's flight (`minds::Mind::turns_on`, #3470, #3622); whether it
    /// does.
    pub fn turns_on(
        &mut self,
        attacker: Option<Entity>,
        busy: bool,
        striking: bool,
        flying: bool,
    ) -> bool {
        let turns = self
            .thinks
            .turns_on(busy || self.opening.is_some(), striking, flying);
        if turns {
            self.foe = attacker;
        }
        turns
    }

    /// A creature sent at the hero (the event `0x100` of a script's `Attack`, #1830); whether it
    /// goes.
    pub fn send_at_hero(&mut self) -> bool {
        let goes = self.thinks.sent_at();
        if goes {
            self.foe = None;
        }
        goes
    }
}

/// The Mage's hero type, a target that a creature of the mark 4 takes its special at the more
/// (`0x4D5CC1`, #3263).
const MAGE: u32 = 3;

/// A creature as another sees it for the fights between them (#630): its entity, where it
/// stands, its side and type, by which they are hostile or not (`minds::hostile`), and its level.
#[derive(Clone, Copy, Debug)]
struct Other {
    entity: Entity,
    at: Vec3,
    side: u8,
    creature: u32,
    /// Its `Creature.pak` class.
    class: u8,
    level: u32,
    /// Its size, of which and another's the reach is the mean (`minds::size`).
    size: u32,
    /// Whether its sides are turned over (`minds::Mind::turned`).
    turned: bool,
}

impl Other {
    fn of(entity: Entity, at: Vec3, mind: &Mind) -> Self {
        Self {
            entity,
            at,
            side: mind.side,
            creature: mind.creature,
            class: mind.class,
            level: mind.level,
            size: mind.size(),
            turned: mind.turned,
        }
    }

    /// Whether `mind` is hostile to it (`0x549000`), the other way about where either's sides
    /// are turned over (`0x423580` at `0x4238C6`).
    fn hated_by(&self, mind: &Mind) -> bool {
        minds::hostile(mind.side, mind.creature, self.side, self.creature)
            != (self.turned || mind.turned)
    }

    /// Whether it is hostile to `mind`, so turned over too.
    fn hates(&self, mind: &Mind) -> bool {
        minds::hostile(self.side, self.creature, mind.side, mind.creature)
            != (self.turned || mind.turned)
    }
}

/// The rules' point of the ground under world point `point`.
fn spot(point: Vec3) -> Spot {
    Spot::new(point.x, point.z)
}

/// The world point over the rules' point of the ground `spot`, at height `y`.
fn point(spot: Spot, y: f32) -> Vec3 {
    Vec3::new(spot.x, y, spot.y)
}

/// Whether a guard's id is odd, which takes it a torch by night (`0x4EFA50`): the game, whose
/// creatures have no ids of the original's object manager, takes the cell of its home, the sum
/// of its coordinates odd (docs/decisions/ai.md, "The guards' torches").
fn odd(home: Spot) -> bool {
    let (x, y) = paths::cell_of(home.x, home.y);
    (x + y).rem_euclid(2) == 1
}

/// The mark's bit of an attacker whose strike turns a creature that plays its own strike or
/// shot (`0x535DD3`): no hero's.
const TURNS_STRIKERS: u16 = 0x80;

/// Turns each creature struck by an attacker hostile to it on that attacker, where it is hostile
/// to the hero or the hero's follower, no horse ridden and alive (the event 5's handler's tail,
/// `0x535A72`–`0x535C45`; docs/re/ai.md, "Foes among the creatures", #3470, #3590), a follower
/// by a creature alone, as the hero is no foe of its own: it takes the AI's
/// state 4 or 8 against it, unless it is held or in a throw's flight (#3622), a script's order
/// waits for it, it casts or makes a weapon art, or its own strike or shot plays and the
/// attacker's mark has no bit `0x80`. A Fetzer that it
/// turns so from another state while it chooses takes its phase, `0x21B` by a roll of 0 of 3,
/// else `0x217` (`0x535C4B`–`0x535C7F`, #3499); one that fights already keeps its choosing.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn turn_on_attackers(
    mut hits: MessageReader<crate::fight::Hit>,
    mut rolls: ResMut<Minds>,
    heroes: Query<(), With<Body>>,
    mut minds: Query<
        (
            &mut Mind,
            (
                Has<crate::creature_casts::Casting>,
                Has<crate::creature_arts::Ordered>,
                Has<crate::creature_arts::Makes>,
                Option<&crate::npcs::ScriptQueue>,
            ),
            Has<crate::npcs::Strikes>,
            Option<&crate::followers::Follower>,
            (
                Option<&crate::making::Knocked>,
                Has<crate::backbreaker::Flung>,
            ),
        ),
        (Without<crate::npcs::Dies>, Without<crate::riding::Ridden>),
    >,
) {
    for hit in hits.read() {
        if hit.attacker == hit.target {
            continue;
        }
        let by_hero = heroes.contains(hit.attacker);
        let attacker = if by_hero {
            None
        } else {
            match minds.get(hit.attacker) {
                Ok((mind, ..)) => Some((mind.thinks.clone(), mind.mark())),
                Err(_) => continue,
            }
        };
        let Ok((mut mind, (casting, ordered, makes, queue), striking, follower, (knocked, flung))) =
            minds.get_mut(hit.target)
        else {
            continue;
        };
        // The hero's follower (`+0x251` the local player), whom the hero's own blows turn not.
        let follows_hero = follower.is_some_and(|follower| heroes.contains(follower.master));
        if !mind.hostile() && !(follows_hero && !by_hero) {
            continue;
        }
        let hostile_to_it = attacker
            .as_ref()
            .is_none_or(|(other, _)| mind.thinks.hostile_to(other));
        if !hostile_to_it {
            continue;
        }
        let busy = casting || ordered || makes || queue.is_some_and(|queue| !queue.is_empty());
        let breaks = attacker
            .as_ref()
            .is_some_and(|(_, mark)| mark & TURNS_STRIKERS != 0);
        let foe = (!by_hero).then_some(hit.attacker);
        let calm = !mind.thinks.fighting();
        // In a throw's flight: the Dwarf's 1083's or the backbreaker's (`+0x14` bit `0x100000`).
        let flying = flung || knocked.is_some_and(|knock| knock.throw.is_some());
        if mind.turns_on(foe, busy, striking && !breaks, flying) {
            debug!(
                "ai: {} turns on {}, which struck it",
                hit.target, hit.attacker
            );
            if calm && mind.mark() == opensacred_rules::casts::FETZER_CHOOSES {
                let roll = rolls.rng.below(3);
                mind.fetzer_struck(roll);
                debug!(
                    "ai: {} is struck and takes its phase {:#x}",
                    hit.target,
                    mind.mark()
                );
            }
        }
    }
}

/// Sends off each struck creature neither hostile to the hero nor her follower that fights a
/// living creature, at every blow, a hit or a miss, whatever it does: its own strike in play cut
/// off, it runs 700 beyond its place away from its own target, its fight's flight, and its fight
/// takes it back once its run ends ([`pursue`]; the event 5's tail, `0x535DDA`–`0x535DE8`, and
/// the flight's order `0x539F60(0)`; docs/re/ai.md, "Foes among the creatures", "What the order
/// comes to", #3591). A horse, an animal and one with no living target stand, as the original's
/// melee state stops them at once (`opensacred_rules::minds::Mind::runs_off_struck`); one in a
/// throw's flight takes its throw first.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn run_off(
    mut commands: Commands,
    mut hits: MessageReader<crate::fight::Hit>,
    mut rolls: ResMut<Minds>,
    heroes: Query<(), With<Body>>,
    mut minds: Query<
        (
            &mut Mind,
            &Transform,
            &InTheWay,
            Option<&crate::followers::Follower>,
            Has<crate::riding::Horse>,
            (
                Option<&crate::making::Knocked>,
                Has<crate::backbreaker::Flung>,
            ),
        ),
        (Without<crate::npcs::Dies>, Without<crate::riding::Ridden>),
    >,
    foes: Query<(&Transform, &crate::fight::Health), Without<crate::npcs::Dies>>,
) {
    for hit in hits.read() {
        if hit.attacker == hit.target {
            continue;
        }
        let Ok((mut mind, at, way, follower, horse, (knocked, flung))) = minds.get_mut(hit.target)
        else {
            continue;
        };
        if flung || knocked.is_some_and(|knock| knock.throw.is_some()) {
            continue;
        }
        let follows_hero = follower.is_some_and(|follower| heroes.contains(follower.master));
        let foe = mind
            .foe()
            .and_then(|foe| foes.get(foe).ok())
            .filter(|(_, health)| health.current > 0);
        if !mind.runs_off_struck(follows_hero, horse, foe.is_some()) {
            continue;
        }
        let Some((from, _)) = foe else {
            continue;
        };
        let here = at.translation;
        let to = minds::runs_off(spot(here), spot(from.translation), &mut rolls.rng);
        debug!(
            "ai: {} is struck and runs off from its target to {:.0},{:.0}",
            hit.target, to.x, to.y
        );
        mind.ordered_off();
        commands
            .entity(hit.target)
            .remove::<Strikes>()
            .insert(Walk::run(point(to, here.y), way.level, mind.run));
    }
}

/// Fells each swooping flyer that a blow of some damage struck aloft, by a roll of 0 to 100 below
/// `⌊100·d/(h + d)⌋` of the blow's damage `d` and its health left `h` (the event 5's handler's
/// tail, `0x535C89`–`0x535D89`; docs/re/ai.md, "Foes among the creatures", #3589): it is held 3
/// s as a knock-back holds, stops and plays its slot 173 once, its swoop starting again from its
/// pass, its flyer's bit kept; its hop from where it is, some 4 units high, is not followed.
fn swoop_falls(
    mut commands: Commands,
    mut blows: MessageReader<crate::fight::Struck>,
    mut rolls: ResMut<Minds>,
    mut minds: Query<(&mut Mind, &crate::fight::Health), Without<crate::npcs::Dies>>,
) {
    for blow in blows.read() {
        let Ok((mut mind, health)) = minds.get_mut(blow.target) else {
            continue;
        };
        if !mind.swooping() || blow.damage == 0 {
            continue;
        }
        let chance = minds::swoop_fall_chance(blow.damage, health.current);
        let roll = rolls.rng.below(101);
        if !minds::swoop_falls(roll, chance) {
            continue;
        }
        debug!(
            "ai: {} falls from its swoop, {roll} below {chance}",
            blow.target
        );
        mind.falls_from_swoop();
        mind.held_at_least(minds::SWOOP_FALL_HOLD);
        commands
            .entity(blow.target)
            .remove::<(Walk, Strikes)>()
            .insert(crate::npcs::ScriptClip::fidget(SWOOP_FALL_SLOT));
    }
}

/// The clip a felled swooping flyer plays once (`*_DYING_A`, `0x535D60`).
const SWOOP_FALL_SLOT: u16 = 173;

/// A swooping flyer's pass that struck its target ([`pass_terror`]).
#[derive(Message, Clone, Copy, Debug)]
pub(crate) struct SwoopPassed {
    flyer: Entity,
    target: Entity,
}

/// The hold of a creature's plant cage, in seconds, where its caster is no hero (`0x5B5CC8`).
const CREATURES_CAGE_HOLDS: f32 = 3.0;

/// The terror of each swooping flyer's pass on its target after its strike (`0x4E25E0`–`0x4E28A4`;
/// docs/re/ai.md, "The flyers", #3597; [`minds::pass_terror`]): where the target's items give
/// the effect 855, or set 41's four pieces on it make `R(0, 100)` fall below `⌊20·D⌋` (M16), the
/// flyer stops what it does and is set down at once ([`minds::Mind::set_down`], `flight`); with
/// 855 a second `R(0, 100)` below its value makes ten plant cages about it, silent, their caster
/// the target: a hero's of the spell 40's value 0 at a third of its level, else 3 s
/// (`plant_cages`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn pass_terror(
    mut commands: Commands,
    mut passes: MessageReader<SwoopPassed>,
    mut rolls: ResMut<Minds>,
    arts: Option<Res<crate::arts::HeroArts>>,
    targets: Query<(
        &crate::fight::Holding,
        Option<&crate::fight::Combat>,
        Option<&crate::fight::Health>,
        Has<Body>,
    )>,
    mut flyers: Query<&mut Mind, Without<crate::npcs::Dies>>,
) {
    for pass in passes.read() {
        let Ok((holding, combat, health, hero)) = targets.get(pass.target) else {
            continue;
        };
        let Ok(mut mind) = flyers.get_mut(pass.flyer) else {
            continue;
        };
        let caging = opensacred_rules::fight::item_effect(
            &holding.0.items,
            opensacred_rules::fight::CAGING,
            crate::drops::attributes_of(combat),
            None,
        );
        let faces = holding.0.sets.terror_faces();
        let terror = if caging > 0.0 {
            0
        } else {
            rolls.rng.below(101)
        };
        let cage = if caging > 0.0 {
            rolls.rng.below(101)
        } else {
            0
        };
        let done = minds::pass_terror(caging, faces, [terror, cage]);
        if done == minds::PassTerror::Flies {
            continue;
        }
        debug!(
            "ai: {} set down by its pass at {}, {done:?}",
            pass.flyer, pass.target
        );
        mind.set_down();
        commands.entity(pass.flyer).remove::<(Walk, Strikes)>();
        if done != minds::PassTerror::Caged {
            continue;
        }
        let seconds = if hero {
            let level = health.map_or(1, |health| health.level) / 3;
            arts.as_deref()
                .and_then(|arts| {
                    arts.tables
                        .spells
                        .iter()
                        .find(|spell| spell.id == opensacred_rules::spells::CAGE)
                })
                .map_or(CREATURES_CAGE_HOLDS, |record| {
                    crate::spells::fight_seconds(opensacred_rules::spells::value(record, 0, level))
                })
        } else {
            CREATURES_CAGE_HOLDS
        };
        crate::plant_cages::cage_silently(
            &mut commands,
            pass.flyer,
            minds::PASS_CAGES,
            (hero, seconds),
        );
    }
}

/// A creature's stand-in for the original's object id, which a hornet's flank and the queen's
/// circle take (#2562): the sum of its home cell's coordinates, as for the guards' torches
/// (docs/decisions/ai.md, "The guards' torches").
fn stand_in_id(home: Spot) -> u32 {
    let (x, y) = paths::cell_of(home.x, home.y);
    u32::try_from((x + y).rem_euclid(1 << 16)).unwrap_or(0)
}

/// The game's generator for the creatures' chances.
#[derive(Resource)]
struct Minds {
    rng: Rng,
}

impl Default for Minds {
    fn default() -> Self {
        Self {
            rng: crate::seed::dice(0),
        }
    }
}

/// A step to `target` on `way`'s level at `speed`, the idle action's, with the AI's state 0.
fn walk(target: Vec3, speed: f32, way: &InTheWay) -> Walk {
    Walk::to(target, way.level, speed).idle()
}

/// The sector of world point `p`.
fn sector_of(p: Vec3) -> (f32, f32) {
    let side = crate::iso::CELL * 64.0;
    ((p.x / side).floor(), (p.z / side).floor())
}

/// A door as a creature in its way sees it, with its entity, its world point, its points and
/// its type.
type Seen = DoorSeen<(Entity, Vec3, Option<DoorPoints>, u32)>;

/// The doors that the creatures in their way look at (`0x4D4800`), at their logical positions
/// (`npcs::Object`), and the hero's marks on them.
#[derive(SystemParam)]
struct Doors<'w, 's> {
    #[expect(
        clippy::type_complexity,
        reason = "a Bevy query's components and filters"
    )]
    doors: Query<
        'w,
        's,
        (
            Entity,
            &'static Object,
            &'static Door,
            Option<&'static Swing>,
            &'static Layered,
        ),
        Without<Mind>,
    >,
    npcs: Option<Res<'w, Npcs>>,
    /// The riders' seats, by which a rider goes for a gate alone (#2293).
    seats: Query<'w, 's, &'static crate::riding::Seat>,
}

impl Doors<'_, '_> {
    /// Whether `creature` rides a horse.
    fn rides(&self, creature: Entity) -> bool {
        self.seats
            .get(creature)
            .is_ok_and(|seat| seat.ridden().is_some())
    }

    /// Each door, where it stands and on which layer, its points ([`door_points`]), whether it
    /// is open, a door turned this tick as it will stand (`npcs::swing` turns it later in the
    /// frame), whether its trigger is barred, the bit 15 that the scripts' `SetObjState` sets
    /// (`npcs::Npcs::locked`, #1369), and whether a hero marked it.
    fn seen(&self, heights: &Heights) -> Vec<Seen> {
        self.doors
            .iter()
            .map(|(entity, object, door, swing, layered)| DoorSeen {
                door: (
                    entity,
                    object.at,
                    self.npcs
                        .as_ref()
                        .and_then(|npcs| door_points(door, npcs.door_cell(door), heights)),
                    door.kind(),
                ),
                at: spot(object.at),
                layer: layered.layer,
                open: swing.map_or(door.open, |swing| swing.open),
                barred: self.npcs.as_ref().is_some_and(|npcs| npcs.locked(door)),
                marked: self.npcs.as_ref().is_some_and(|npcs| npcs.marked(door)),
            })
            .collect()
    }
}

/// The door in the way of a creature at `here` on `layer`, of the doors of its sector, as the
/// rules choose it (`opensacred_rules::minds::door_in_the_way`, `0x4D4800`).
fn door_in_way(
    here: Vec3,
    layer: i32,
    doors: &[Seen],
) -> Option<(Entity, Vec3, Option<DoorPoints>, u32)> {
    let sector = sector_of(here);
    let ours = doors
        .iter()
        .copied()
        .filter(|door| sector_of(door.door.1) == sector);
    door_in_the_way(spot(here), layer, ours)
}

/// The points of `door` (`opensacred_rules::ways::DoorPoints`, docs/re/ai.md, "Houses and
/// doors", #431), from its trigger's cell `spot` and its leaf's direction: the cells beside it
/// walkable on its building's floor and on the ground; a door on a floor above the first has that
/// floor on both sides.
pub(crate) fn door_points(
    door: &Door,
    spot: Option<(i32, i32, i32)>,
    heights: &Heights,
) -> Option<DoorPoints> {
    let (x, y, floor) = spot?;
    let floor = usize::try_from(floor).unwrap_or(0).max(1);
    let walkable = |cell: (i32, i32), on_floor: bool| {
        if on_floor {
            heights
                .building_on_floor(cell, floor)
                .map(|building| heights.floor_level(building, floor))
                .is_some_and(|level| level.is_some() && heights.walkable_on(cell, level))
        } else {
            floor == 1 && heights.walkable_on(cell, None)
        }
    };
    let along = (door.along.x, door.along.y);
    Some(DoorPoints::of((x, y), along, walkable))
}

/// The point at which a creature at `here` on `level` stands to open a door (`0x53A170`,
/// `0x43B290`): its outer point on the ground and its inner on a building's floor, or the one on
/// its own side of a door with the same on both (`points`, #431); where the door has none, the
/// cell beside the door's that may be walked on at the creature's level, the nearest to it, or
/// the door itself where there is none.
pub(crate) fn door_point(
    points: Option<DoorPoints>,
    door: Vec3,
    here: Vec3,
    level: Level,
    heights: &Heights,
) -> Vec3 {
    let cell = crate::iso::CELL;
    points
        .and_then(|points| points.near((here.x / cell, here.z / cell), level.is_some()))
        .and_then(centre)
        .map_or_else(
            || beside(door, here, level, heights),
            |at| at.with_y(here.y),
        )
}

/// The cell beside the door at `door` that may be walked on at `level`, the nearest to a creature
/// at `here`, or the door itself where there is none: where a door has no points.
fn beside(door: Vec3, here: Vec3, level: Level, heights: &Heights) -> Vec3 {
    let (x, y) = paths::cell_of(door.x, door.z);
    opensacred_rules::ways::STEPS
        .iter()
        .map(|&(dx, dy)| (x + dx, y + dy))
        .filter(|&cell| heights.walkable_on(cell, level))
        .filter_map(centre)
        .min_by(|a, b| {
            let (da, db) = ((*a - here).xz().length(), (*b - here).xz().length());
            da.total_cmp(&db)
        })
        .unwrap_or(door)
        .with_y(here.y)
}

/// Sends the creature `entity` at `here` to the door in its way, `door` at `at` with its
/// `points` (the event 7 with the door, the AI's state 1, `0x53A170`): it walks to the door's
/// point on its side ([`door_point`]), stopping within 20, or stands where it is within 25 of
/// it along each axis, and opens it standing ([`open`]).
fn go_for_door(
    commands: &mut Commands,
    (entity, here, way, rides): (Entity, Vec3, &InTheWay, bool),
    mind: &mut Mind,
    (door, at, points, kind): (Entity, Vec3, Option<DoorPoints>, u32),
    heights: &Heights,
) {
    // A rider goes for a gate alone; another door gets it off its horse, its mount its target
    // (`0x53A170`, #2293), and on foot it goes for the door at its next decision.
    if rides && !opensacred_rules::minds::rides_through(kind) {
        debug!("ai: {entity} rides to {door} in its way and gets off its horse");
        commands.entity(entity).insert(crate::npcs::GetsOff);
        return;
    }
    let target = door_point(points, at, here, way.level, heights);
    mind.opening = Some(door);
    let off = (target - here).abs();
    if off.x <= AT_THE_DOOR_POINT && off.z <= AT_THE_DOOR_POINT {
        debug!("ai: {entity} stands at its point of {door} in its way");
        return;
    }
    debug!(
        "ai: {entity} goes for {door} in its way, from {:.0},{:.0} to {:.0},{:.0}",
        here.x, here.z, target.x, target.z
    );
    commands
        .entity(entity)
        .insert(Walk::to(target, way.level, mind.speed).within(AT_THE_DOOR));
}

/// Whether a way leads from a creature at `here` on `level` to the cell of `to` on any level,
/// round the cells that may not be walked on and through the doors, the creatures not counted
/// (`opensacred_rules::ways::path_onto`, `0x4F7710`): to a cell brought nearer along the line
/// where `to` lies farther than 25 cells along an axis (`opensacred_rules::ways::nearer`,
/// `0x4FABE0`).
pub(crate) fn way_to(here: Vec3, level: Level, to: Spot, heights: &Heights) -> bool {
    way_onto(here, level, (to, None), heights)
}

/// [`way_to`], onto the level `onto` where it is given: a follower's step off's floor (#1013).
pub(crate) fn way_onto(
    here: Vec3,
    level: Level,
    (to, onto): (Spot, Option<Level>),
    heights: &Heights,
) -> bool {
    let from = paths::cell_of(here.x, here.z);
    let to = opensacred_rules::ways::nearer(from, paths::cell_of(to.x, to.y), |cell| {
        heights.walkable_on(cell, level)
    });
    paths::path_onto(
        (from, (level, false)),
        to,
        |(at, _)| onto.is_none_or(|onto| at == onto),
        |place| heights.neighbours(place),
    )
    .is_some()
}

/// Whether the launch test of a spell of the launch type `launch` wants its target on its caster's
/// level (`0x51D840` at `0x449930`): one on an enemy (1), at a point of the ground (3), whose
/// building's floor there is the caster's, or on the dead (4); one on a friend (2) is tested once
/// the cast chooses its friend (#3857), and one on itself (5) or of none (0) never (#3852).
fn levels_tested(launch: u16) -> bool {
    matches!(launch, 1 | 3 | 4)
}

/// Whether a straight line of a creature on the floor number `floor` passes `cell` by `test`, the
/// walker's or the flyer's, on the level that the original's line tests there: that floor of the
/// building of the cell on the ground, the ground where the cell is no building's, closed where
/// the building has no such floor (`terrain::Heights::line_level`, `0x4F7DD0`; docs/re/fight.md,
/// "Across a building's levels", #3852). So a line from a floor passes out through an open door
/// onto the ground beyond, and over a floor's grid above ground of no building it tests the
/// ground.
fn line_passes(
    heights: &Heights,
    floor: usize,
    cell: (i32, i32),
    test: impl Fn((i32, i32), Level) -> bool,
) -> bool {
    heights
        .line_level(cell, floor)
        .is_some_and(|at| test(cell, at))
}

/// Each creature that went for a door in its way, once it stands, opens it within 300 of it on
/// its layer or the ground and the first floor either way (`opensacred_rules::minds::opens`):
/// the event `0x21` of a creature that is no hero (`Swing::to`), which opens the closed door
/// that a hero marked; then it takes the AI's state 0 again, whose choice looks for the hero
/// anew (`0x53A170`).
fn open(
    mut commands: Commands,
    doors: Query<(&Object, &Layered), With<Door>>,
    mut creatures: Query<(Entity, &Transform, &Layered, &mut Mind), Without<Walk>>,
) {
    for (entity, transform, layered, mut mind) in &mut creatures {
        let Some(door) = mind.opening.take() else {
            continue;
        };
        mind.left_door();
        let Ok((at, door_layer)) = doors.get(door) else {
            continue;
        };
        let here = transform.translation;
        if opens(spot(here), layered.layer, spot(at.at), door_layer.layer) {
            debug!("ai: {entity} opens {door} in its way");
            commands.entity(door).insert(Swing::to(true));
        } else {
            let d = (at.at - here).xz().length();
            debug!("ai: {entity} stands {d:.0} from {door}, too far to open it");
        }
    }
}

/// The Dark Elf (the type 4).
const DARK_ELF: u32 = 4;

/// Whether the hero of the type `kind` goes unseen by the creatures and steps through them
/// (`0x54A6D0`; docs/re/movement.md, "The unseen walker", #3084), `attacking` nothing (no attack
/// ordered and no strike, cast or shot playing: the water form's AI's state not 4, 6 or 8 and its
/// action not 3, 4 or 5, the Dark Elf's action 0, 1 or 2 and its AI's state not 4, 6, 8 or 11):
/// in the water form (`spells::Unseen`), or the Dark Elf of seven pieces of set 26
/// (`set_bonuses::Stats::unseen`, #1727's M9) that `holding` gives.
pub(crate) fn unseen_hero(
    kind: Option<u32>,
    (unseen, holding): (
        Option<&crate::spells::Unseen>,
        Option<&crate::fight::Holding>,
    ),
    attacking: bool,
) -> bool {
    !attacking
        && (unseen.is_some_and(|unseen| unseen.holds())
            || kind == Some(DARK_ELF) && holding.is_some_and(|holding| holding.0.sets.unseen()))
}

/// Runs the creatures' timers, the wakes of the hero's look (`crowd::look`), and the awake
/// creatures' thought (`opensacred_rules::minds::Mind::think`), and walks them where it sends
/// them: a creature without a straight line to the hero to the door in its way, if it finds one.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn think(
    mut commands: Commands,
    tick: Res<crate::frame::WorldTick>,
    mut minds: ResMut<Minds>,
    nearest: Res<Nearest>,
    heights: Res<Heights>,
    (balance, rules): (Res<FightRules>, Option<Res<crate::fight::Rules>>),
    experience: Res<crate::fight::Experience>,
    doors: Doors,
    hero: Query<(
        &Transform,
        &Body,
        Option<&crate::hero::Size>,
        Option<&crate::spells::Unseen>,
        (
            Has<crate::attack::Striking>,
            Option<&crate::attack::Attack>,
            Option<&crate::fight::Holding>,
        ),
    )>,
    kind: Option<Res<crate::hero::Hero>>,
    mut creatures: Query<
        (
            Entity,
            &Transform,
            &InTheWay,
            &mut Mind,
            Has<Walk>,
            &Layered,
            Has<crate::making::Knocked>,
            Option<&crate::effects::Effects>,
        ),
        (Without<Body>, Without<crate::npcs::Dies>),
    >,
    (queued, npcs, mut shots, heroes): (
        Query<
            (),
            Or<(
                With<crate::npcs::ScriptQueue>,
                With<crate::npcs::ScriptClip>,
            )>,
        >,
        Query<&crate::npcs::Npc>,
        Query<&mut Strikes>,
        Query<Entity, With<Body>>,
    ),
) {
    let Some((hero, body, size, unseen, (striking, attack, holding))) = hero.iter().next() else {
        return;
    };
    let attacking = striking || attack.and_then(crate::attack::Attack::target).is_some();
    let unseen = unseen_hero(
        kind.as_deref().map(|hero| hero.creature),
        (unseen, holding),
        attacking,
    );
    // The creatures that fight, which an idle hostile creature of their sector may take for its
    // foe (`0x54338B`).
    let fighters: Vec<Other> = creatures
        .iter()
        .filter(|(.., mind, _, _, _, _)| mind.fighting())
        .map(|(entity, transform, _, mind, ..)| Other::of(entity, transform.translation, mind))
        .collect();
    // The hero's list of hostiles, nearest first, among which the Underworld's pairs look for
    // each other (`0xAD3CE0`, #2905).
    let listed_others: Vec<Other> = nearest
        .hostiles
        .iter()
        .filter_map(|&other| creatures.get(other).ok())
        .map(|(entity, transform, _, mind, ..)| Other::of(entity, transform.translation, mind))
        .collect();
    let hero = hero.translation;
    let hero_size = size.map_or(opensacred_rules::minds::SIZE, |size| size.0);
    let dt = tick.seconds.unwrap_or_default();
    let minds = &mut *minds;
    let look = nearest.looked;
    let side = crate::iso::CELL * 64.0;
    let sector = |p: Vec3| ((p.x / side).floor(), (p.z / side).floor());
    let (sx, sy) = sector(hero);
    let hero_moves = body.moving().is_some();
    // The steps taken this tick: a creature, its target, and its group's step to share; the
    // groups whose alarm goes.
    let mut steps: Vec<(Entity, Vec3)> = Vec::new();
    let mut fidgets: Vec<(Entity, minds::Fidget)> = Vec::new();
    let mut alarms: Vec<u32> = Vec::new();
    let mut grouped: Vec<(u32, Vec3)> = Vec::new();
    let seen = doors.seen(&heights);
    // A creature's level on bronze's scale, which it weighs against its foe's (`0x5522D0`).
    let on_bronze = |level| rules.as_ref().map_or(level, |rules| rules.on_bronze(level));
    for (entity, transform, way, mut mind, walking, layered, sliding, effects) in &mut creatures {
        // A knocked creature thinks nothing while it slides, but for its timer while it is held
        // (`opensacred_rules::minds::Mind::knocked`).
        if sliding && !mind.held() {
            continue;
        }
        // One whose script's queue runs or whose script's clip plays leaves its idle thought,
        // fighting still where it fights (`0x542B20` at `0x542D9E`, #1220); its timer runs on all
        // the same, as the creatures' update runs it, so that a hold ends while a clip plays
        // (`0x53947B`, #3628).
        if queued.contains(entity) && !mind.fighting() {
            mind.run_timer(dt, sliding, &mut minds.rng);
            continue;
        }
        let here = transform.translation;
        let (x, y) = sector(here);
        // The creatures of its sector that fight, of another side, hostile to it, which an idle
        // hostile creature may take for its foe, and whether it takes each on by its level.
        // One whose sides are turned over takes its old friends so, its own side among them.
        let candidates: Vec<&Other> = if (mind.hostile() || mind.turned) && !mind.fighting() {
            fighters
                .iter()
                .filter(|other| {
                    other.entity != entity
                        && (other.side != mind.side || other.turned || mind.turned)
                        && sector(other.at) == (x, y)
                        && other.hated_by(&mind)
                })
                .collect()
        } else {
            Vec::new()
        };
        let foes: Vec<Foe> = candidates
            .iter()
            .map(|other| Foe {
                at: spot(other.at),
                takes_on: crate::fight::takes_on(
                    on_bronze(mind.level),
                    other.level,
                    balance.experience,
                ),
                size: other.size,
            })
            .collect();
        // The hero's hostiles but itself, as foes and as the Underworld's pairs see them, and its
        // own distance from the hero, where it is in the list, else the first's.
        let hostiles: Vec<&Other> = if (mind.hostile() || mind.turned) && !mind.fighting() {
            listed_others
                .iter()
                .filter(|other| other.entity != entity)
                .collect()
        } else {
            Vec::new()
        };
        let listed: Vec<(Foe, minds::Listed)> = hostiles
            .iter()
            .map(|other| {
                (
                    Foe {
                        at: spot(other.at),
                        takes_on: crate::fight::takes_on(
                            on_bronze(mind.level),
                            other.level,
                            balance.experience,
                        ),
                        size: other.size,
                    },
                    minds::Listed {
                        creature: other.creature,
                        class: other.class,
                        hostile: other.hated_by(&mind) || other.hates(&mind),
                        distance: (other.at - here).xz().length(),
                    },
                )
            })
            .collect();
        let own = listed_others
            .iter()
            .find(|other| other.entity == entity)
            .or(listed_others.first())
            .map_or(0.0, |other| (other.at - hero).xz().length());
        // A creature that the Vampiress dominates does not see her for a foe (the effect 13), nor
        // any the hero in its water form (`0x54A6D0`).
        let dominated = unseen
            || effects.is_some_and(|effects| effects.holds(opensacred_rules::arts::DOMINATED));
        let senses = Senses {
            dt,
            seen: look && (x - sx).abs() <= 1.0 && (y - sy).abs() <= 1.0 && !dominated,
            here: spot(here),
            hero: spot(hero),
            hero_moves,
            takes_on: crate::fight::takes_on(
                on_bronze(mind.level),
                experience.level,
                balance.experience,
            ),
            walking,
            sliding,
            hero_size,
            foes: &foes,
            listed: &listed,
            own,
        };
        // A line of sight to its target, the straight way's test with the flyer's flag, over the
        // water and the fences (`0x4FCC80` from `0x544138`, #3358), on its floor of the building
        // at each cell (#3852).
        let floor = heights.floor_number(way.level);
        let clear = |to: Spot| {
            paths::straight(here.xz(), Vec2::new(to.x, to.y), |cell| {
                line_passes(&heights, floor, cell, |cell, at| {
                    heights.flown_onto(cell, at)
                })
            })
        };
        // The door in its way, if no straight line leads to the hero (`0x5442E8`).
        let mut found = None;
        let door = || {
            found = door_in_way(here, layered.layer, &seen);
            found.is_some()
        };
        let thought = mind.think(&senses, clear, door, &mut minds.rng);
        if let (true, Some(found)) = (thought.door, found) {
            go_for_door(
                &mut commands,
                (entity, here, way, doors.rides(entity)),
                &mut mind,
                found,
                &heights,
            );
        }
        if let Some(home) = thought.home {
            steps.push((entity, point(home, here.y)));
        }
        if thought.attacks {
            // A fighter, else one of the hero's hostiles, its index after the fighters'.
            let picked = thought.foe.and_then(|index| {
                candidates
                    .get(index)
                    .or_else(|| hostiles.get(index.checked_sub(candidates.len())?))
                    .copied()
            });
            mind.foe = picked.map(|other| other.entity);
            match picked {
                Some(foe) => {
                    let distance = (here - foe.at).xz().length();
                    debug!(
                        "ai: {entity} {:?} {}, a {} that fights, from {distance:.0}",
                        Why::Attacks,
                        foe.entity,
                        foe.creature
                    );
                }
                None => {
                    let distance = (here - hero).xz().length();
                    debug!(
                        "ai: {entity} {:?} the hero from {distance:.0}",
                        Why::Attacks
                    );
                }
            }
            if let (true, Some(group)) = (thought.alarm, mind.group) {
                alarms.push(group);
            }
        }
        // A step of a group's member of an `Enter` function moves the whole group, the stepper
        // among them, below (`0x54F2AE`).
        if let Some(fidget) = thought.fidget {
            fidgets.push((entity, fidget));
        }
        if let Some(step) = thought.step {
            let target = point(step, here.y);
            match (mind.group, mind.enter) {
                (Some(group), true) => grouped.push((group, target - here)),
                _ => steps.push((entity, target)),
            }
        }
    }
    // A group's members, the stepper among them, step by the same step, each within 50 more and
    // its home within 50 of its target; a walking member is sent anew (`0x54F2AE` to `0x54F725`).
    for (group, step) in grouped {
        for (entity, transform, _, mut mind, _, _, sliding, _) in &mut creatures {
            // Not one that goes for a door, nor one knocked back.
            if mind.group != Some(group) || mind.to_door() || sliding || mind.held() {
                continue;
            }
            let here = transform.translation;
            let target = mind.fellow_steps(spot(here), spot(step), &mut minds.rng);
            // A hostile one of the guards' mode takes its behaviour instead, its home moved
            // (#3073).
            if mind.guards_instead() {
                continue;
            }
            steps.push((entity, point(target, here.y)));
        }
    }
    // A fidget plays its clip once where it stands (the standing action's sub-state `0x65`,
    // `0x54EA30`, #1364): its model's fun clip, or its attack's.
    for (entity, fidget) in fidgets {
        if let Ok(npc) = npcs.get(entity) {
            let slot = npc.fidget_slot(fidget);
            debug!("ai: {entity} fidgets with the slot {slot}");
            commands
                .entity(entity)
                .insert(crate::npcs::ScriptClip::fidget(slot));
        }
    }
    for (entity, target) in steps {
        if let Ok((_, _, way, mind, ..)) = creatures.get(entity) {
            commands
                .entity(entity)
                .insert(walk(target, mind.speed, way));
        }
    }
    // The alarm sends a group's idle hostile members at the hero too; a member in mid-shot
    // takes it in its shots' chain and aims its playing shot at the hero (docs/re/fight.md,
    // "The shots' chain", #3313).
    let the_hero = heroes.iter().next();
    for group in alarms {
        for (entity, _, _, mut mind, ..) in &mut creatures {
            if let Ok(mut strikes) = shots.get_mut(entity)
                && strikes.is_shot()
            {
                if mind.called_mid_shot(group)
                    && let Some(the_hero) = the_hero
                {
                    debug!(
                        "ai: {entity} is called in mid-shot, its chain {}",
                        mind.chain()
                    );
                    strikes.retarget(the_hero);
                }
            } else if !mind.fighting() {
                mind.hears_alarm(group);
                mind.foe = None;
            }
        }
    }
}

/// The balance's values of the fight that the AI takes: the threshold of the test of the levels
/// and its full share (`UnterExpAnteil`, `UnterExpVoll`).
#[derive(Resource, Clone, Copy, Debug)]
pub struct FightRules {
    pub experience: (f32, f32),
}

impl Default for FightRules {
    /// The original's values of bronze, where the balance lacks them (docs/re/fight.md).
    fn default() -> Self {
        Self {
            experience: (0.85, 1.0),
        }
    }
}

/// Each creature that fights the hero keeps the band of its reach, flees wounded and goes home
/// from too far, as the rules decide (`opensacred_rules::minds::Mind::pursue`, `0x4E0300`): it
/// comes on, after the hero as it moves, steps back, flees and goes home at its walk or run, and
/// in the band stands turned to the hero, where it strikes (#280). The second to the fourth of the
/// hero's list of hostiles (`crowd::Nearest`), deciding near the first, walk to their places
/// about the hero for a tick (`0x4E224E`). One that comes on in melee without a straight line of
/// walkable cells to the hero, and without a way to it, goes for the door in its way, or gives
/// up (`opensacred_rules::minds::Mind::blocked`). One with a ranged weapon keeps 300 to 800 from
/// the hero (`0x4DBFC0`, #391): it backs off, comes on, goes round where no straight line leads to
/// the hero, and shoots, standing turned to the hero. As a strike in melee ends, its next chains
/// or waits for the AI's two steps, as the rules decide (`Mind::ends_strike`, #466).
/// One that fights a creature, its foe (#630), fights it so, in melee or at range (#670), steered
/// by the foe alone without places about it; it leaves the fight once its foe is dead or gone
/// (**hypothesis**).
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn pursue(
    mut commands: Commands,
    mut lines: MessageWriter<crate::voices::Speak>,
    tick: Res<crate::frame::WorldTick>,
    mut minds: ResMut<Minds>,
    (heights, dungeons, crowd): (
        Res<Heights>,
        Option<Res<crate::daynight::Dungeons>>,
        Res<Crowd>,
    ),
    nearest: Res<Nearest>,
    doors: Doors,
    hero: Query<
        (
            Entity,
            &Transform,
            (Has<crate::attack::Striking>, Has<crate::making::Playing>),
            Option<&crate::attack::Attack>,
            Option<&crate::hero::Size>,
            Option<&crate::spells::Unseen>,
            Option<&crate::fight::Holding>,
            Has<crate::hero_held::HeroHeld>,
            Option<&crate::fight::Health>,
            &Body,
        ),
        (Without<crate::npcs::Dies>,),
    >,
    kind: Option<Res<crate::hero::Hero>>,
    mut creatures: Query<
        (
            Entity,
            &mut Transform,
            &InTheWay,
            &mut Mind,
            Option<&Walk>,
            Has<Strikes>,
            Option<&crate::fight::Health>,
            &Layered,
            Has<crate::npcs::StrikeEnded>,
            Option<&crate::effects::Effects>,
            (
                Option<&crate::fight::Ranked>,
                Has<crate::creature_casts::Casting>,
                Option<&crate::creature_casts::Summoned>,
                Option<&crate::followers::Follower>,
                Option<&mut crate::npcs::ShotEnded>,
                Option<&crate::bolt_hands::BoltHands>,
            ),
        ),
        (
            Without<Body>,
            Without<crate::making::Knocked>,
            Without<crate::backbreaker::Flung>,
        ),
    >,
    (real, experience, rules, difficulty, arts): (
        Res<Time<bevy::time::Real>>,
        Option<Res<crate::fight::Experience>>,
        Option<Res<crate::fight::Rules>>,
        Option<Res<crate::game::Difficulty>>,
        Option<Res<crate::arts::HeroArts>>,
    ),
    (clips, scripted, makers, arms): (
        Query<&crate::npcs::Npc>,
        Query<&crate::npcs::ScriptClip>,
        Query<
            (),
            Or<(
                With<crate::creature_arts::Makes>,
                With<crate::creature_arts::Ordered>,
                With<crate::creature_arts::Volley>,
                With<crate::creature_arts::Leaps>,
            )>,
        >,
        Query<&crate::npcs::Arms>,
    ),
    (mut hits, mut passes): (MessageWriter<crate::fight::Hit>, MessageWriter<SwoopPassed>),
) {
    let dt = tick.seconds.unwrap_or_default();
    let Some((
        the_hero,
        hero,
        (hero_strikes, hero_makes),
        attack,
        size,
        unseen,
        holding,
        hero_held,
        hero_health,
        body,
    )) = hero.iter().next()
    else {
        return;
    };
    // The level the hero stands on, against which a creature in melee strikes it (#3852).
    let hero_layer = body.level();
    let hero_health = hero_health.map(|health| (health.current, health.most));
    let attacking = hero_strikes || attack.and_then(crate::attack::Attack::target).is_some();
    let unseen = unseen_hero(
        kind.as_deref().map(|hero| hero.creature),
        (unseen, holding),
        attacking,
    );
    let hero = hero.translation;
    // The creature the hero attacks.
    let attacked = attack.and_then(crate::attack::Attack::target);
    // Where the nearest of the hero's hostiles stands.
    let first = nearest
        .hostiles
        .first()
        .and_then(|&first| creatures.get(first).ok())
        .map(|(_, transform, ..)| spot(transform.translation));
    let seen = doors.seen(&heights);
    // Where each creature stands, whether it casts, the creature it fights, its size, whether it
    // fights and its level: the foes.
    let foes: std::collections::HashMap<Entity, (Vec3, bool, Option<Entity>, u32, bool, Level)> =
        creatures
            .iter()
            .map(|(entity, transform, way, mind, .., (_, casting, ..))| {
                (
                    entity,
                    (
                        transform.translation,
                        casting,
                        mind.foe(),
                        mind.size(),
                        mind.fighting(),
                        way.level,
                    ),
                )
            })
            .collect();
    // The hero's hostiles alive, which a magician aids (#1854): each with its place, whether it
    // is wounded and whether it fights.
    let friends: Vec<(Entity, Vec3, bool, bool)> = creatures
        .iter()
        .filter(|(_, _, _, mind, _, _, health, ..)| {
            mind.hostile() && health.is_some_and(|health| health.current > 0)
        })
        .map(|(entity, transform, _, mind, _, _, health, ..)| {
            let wounded = health.is_some_and(|health| {
                opensacred_rules::casts::wounded(health.current, health.most)
            });
            (entity, transform.translation, wounded, mind.fighting())
        })
        .collect();
    // The creatures not hostile to the hero alive, with their places and their health: the
    // friends of a caster not hostile to him, the hero among them (Sharifa's heal, #1883).
    let allies: Vec<(Entity, Vec3, u32, u32)> = creatures
        .iter()
        .filter_map(|(entity, transform, _, mind, _, _, health, ..)| {
            let health = health.filter(|health| health.current > 0)?;
            (!mind.hostile()).then_some((
                entity,
                transform.translation,
                health.current,
                health.most,
            ))
        })
        .chain(hero_health.map(|(current, most)| (the_hero, hero, current, most)))
        .collect();
    let hero_size = size.map_or(opensacred_rules::minds::SIZE, |size| size.0);
    // The living creatures that each caster's summons made (#1869).
    let mut summoned: std::collections::HashMap<Entity, u32> = std::collections::HashMap::new();
    for (.., health, _, _, _, (_, _, by, ..)) in &creatures {
        if let Some(by) = by
            && health.is_some_and(|health| health.current > 0)
        {
            *summoned.entry(by.0).or_default() += 1;
        }
    }
    // The hero's followers, which a magician's cast takes for the hero (#1869).
    let hero_followers: std::collections::HashSet<Entity> = creatures
        .iter()
        .filter(|(.., (_, _, _, follower, ..))| {
            follower.is_some_and(|follower| follower.master == the_hero)
        })
        .map(|(entity, ..)| entity)
        .collect();
    let now = real.elapsed_secs_f64();
    let hero_level = experience
        .as_deref()
        .map_or(1, |experience| experience.level);
    // The hornets alive in the hero's list (#2562): each one's type and whether it strikes.
    let hornets: Vec<(Entity, u32, bool)> = nearest
        .hostiles
        .iter()
        .filter_map(|&other| {
            let (_, _, _, mind, _, strikes, health, ..) = creatures.get(other).ok()?;
            let alive = health.is_none_or(|health| health.current > 0);
            (alive && opensacred_rules::casts::HORNETS.contains(&mind.creature)).then_some((
                other,
                mind.creature,
                strikes,
            ))
        })
        .collect();
    // The creatures of a group that strike, each its group and its size (#2565).
    let strikers: Vec<(Entity, u32, u32)> = creatures
        .iter()
        .filter(|(other, _, _, _, _, strikes, ..)| *strikes || makers.contains(*other))
        .filter_map(|(other, _, _, mind, ..)| Some((other, mind.group?, mind.size())))
        .collect();
    // The Fetzers of the hero's list that fight the hero (#2568).
    let fetzers: Vec<Entity> = nearest
        .hostiles
        .iter()
        .copied()
        .filter(|&other| {
            creatures.get(other).is_ok_and(|(_, _, _, mind, ..)| {
                matches!(mind.creature, 202 | 344) && mind.fighting() && mind.foe().is_none()
            })
        })
        .collect();
    // The hero's hostiles of the marks 8 and `0x200` themselves that fight hand to hand (the AI's
    // state 4: fighting with no ranged weapon), which a caster's deadline and its holds' roll
    // count (`0x4D4670`, #2944).
    let hand_to_hand = u32::try_from(
        nearest
            .hostiles
            .iter()
            .filter(|&&other| {
                creatures.get(other).is_ok_and(|(_, _, _, mind, ..)| {
                    matches!(mind.mark(), 8 | 0x200) && mind.fighting() && !mind.ranged
                })
            })
            .count(),
    )
    .unwrap_or(u32::MAX);
    // The hero of set 58's six pieces, who strikes down a creature that would flee it (#2547).
    let strikes_fleeing = holding.is_some_and(|holding| holding.0.sets.strikes_fleeing());
    let difficulty = difficulty.map_or_else(Default::default, |difficulty| difficulty.0);
    for (
        entity,
        mut transform,
        way,
        mut mind,
        walk,
        striking,
        health,
        layered,
        ended,
        effects,
        (rank, casting, _, _, shot_ended, hands),
    ) in &mut creatures
    {
        mind.rest(dt);
        if ended {
            let roll = minds.rng.below(4);
            if mind.ends_strike(roll) {
                mind.chains = true;
            }
            commands.entity(entity).remove::<crate::npcs::StrikeEnded>();
        }
        // After its shot it stands dormant until the hero's look of a later tick wakes it, its
        // idle handler takes it up in the look's own tick, and its decisions shoot again a tick
        // later, two for one with no ranged weapon in hand, which goes through the melee state
        // first (`0x542B20`, `0x547070`, `0x4E0300`, `0x4DBFC0`; seen 0.20 to 0.45 s and 0.30 to
        // 0.67 s from a shot's end to the next, #3312, and tick by tick, #3572).
        if let Some(mut wait) = shot_ended {
            wait.woken = match wait.woken {
                None if nearest.looked => Some(0),
                None => continue,
                Some(ticks) => Some(ticks.saturating_add(1)),
            };
            // By its hands, not by its fight at range, which the ice druid's bolts' effects give it
            // (#3571): one with no ranged weapon in hand searches a way to its target as its idle
            // handler takes it up in the look's tick, which sets its standing clip to its pace's
            // rate where the shot's end's blend is over (#3562, #3572), and shoots again a tick
            // later than one with a bow.
            let holds_ranged = arms.get(entity).is_ok_and(crate::npcs::Arms::ranged);
            if wait
                .woken
                .is_some_and(|woken| minds::searches_after_shot(woken, holds_ranged))
            {
                commands.entity(entity).insert(crate::npcs::SearchesWay);
            }
            if wait.woken < Some(minds::shot_wake(holds_ranged)) {
                continue;
            }
            commands.entity(entity).remove::<crate::npcs::ShotEnded>();
        }
        // A parry plays to its end before its creature decides again (the action 0's sub-state
        // `0x65`, #2487).
        // So an art it makes (#2564).
        if scripted
            .get(entity)
            .is_ok_and(crate::npcs::ScriptClip::parries)
            || makers.contains(entity)
        {
            continue;
        }
        let here = transform.translation;
        // Its target: its foe, a creature, or the hero. A foe dead or gone it leaves; the hero it
        // leaves alone while she dominates it (the effect 13), or while its sides are turned over
        // (the effect 14).
        let foe = mind.foe();
        if foe.is_none()
            && (mind.turned
                || unseen
                || effects.is_some_and(|effects| effects.holds(opensacred_rules::arts::DOMINATED)))
        {
            continue;
        }
        // Whether its target makes an art or a spell, which refuses its flight: the hero's art
        // under way, a creature's cast (#2497).
        let (target, aimed, target_makes, listed, targeted, target_size, target_level) = match foe {
            Some(foe) => match foes.get(&foe) {
                Some(&(at, casts, its_foe, size, _, level)) => {
                    (at, foe, casts, None, its_foe == Some(entity), size, level)
                }
                _ => {
                    debug!("ai: {entity} leaves its fight with {foe}, dead or gone");
                    mind.loses_foe();
                    mind.foe = None;
                    continue;
                }
            },
            None => (
                hero,
                the_hero,
                hero_makes,
                nearest
                    .hostiles
                    .iter()
                    .position(|&hostile| hostile == entity)
                    .zip(first),
                attacked == Some(entity),
                hero_size,
                hero_layer,
            ),
        };
        // A walk begun outside the fight (an idle step, a walk home) it ends as it fights: the
        // original's sub-state 1 points its move at the target at once (`0x4E2066`–`0x4E20FA`,
        // #2489); the fight's own walks run on (#387).
        let walk = match walk {
            Some(walk) if walk.is_idle() && mind.fighting() => {
                commands.entity(entity).remove::<Walk>();
                None
            }
            walk => walk,
        };
        let sight = Pursuit {
            here: spot(here),
            hero: spot(target),
            walk: walk.map(|walk| Walking {
                target: spot(walk.target()),
                comes_on: walk.comes_on(),
                runs: walk.runs(),
            }),
            striking,
            health: health.map(|health| (health.current, health.most)),
            target_makes,
            listed,
            targeted,
            hero_size: target_size,
            // On the ground outside a dungeon, where a crow fleeing takes off (#2291).
            outdoors: way.level.is_none()
                && !dungeons
                    .as_deref()
                    .is_some_and(|dungeons| dungeons.holds(here.x, here.z)),
            // Its level on bronze's scale against the hero's (`0x4E1061`).
            strikes_down: strikes_fleeing
                && aimed == the_hero
                && rules.as_deref().is_some_and(|rules| {
                    minds::struck_down(hero_level, difficulty, rules.on_bronze(mind.level))
                }),
            levels_meet: target_level == way.level,
        };
        // The hero itself, which a magician casts at in place of its follower (#1869).
        let heros_place = hero;
        let hero = target;
        // A straight line of walkable cells to its target, asked again below (`0x4FCC80`), and a
        // line of sight, the test with the flyer's flag, over the water and the fences: the fight
        // at range's (`0x4DC89C`) and the brain-footers' stare's (`0x4D9D7C`; #3358).
        // Each on its floor of the building at each cell (#3852).
        let floor = heights.floor_number(way.level);
        let clear = || {
            paths::straight(here.xz(), hero.xz(), |cell| {
                line_passes(&heights, floor, cell, |cell, at| {
                    heights.walkable_on(cell, at)
                })
            })
        };
        let sighted = || {
            paths::straight(here.xz(), hero.xz(), |cell| {
                line_passes(&heights, floor, cell, |cell, at| {
                    heights.flown_onto(cell, at)
                })
            })
        };
        // And a walk's line that the creatures on it stop but its target (`0x4FCC80` with the
        // target's id): the melee's beyond its band (`0x4E4887`) and the big demon's standing
        // cast (`0x4D5E0B`; #3360).
        let walked = || {
            paths::straight(here.xz(), hero.xz(), |cell| {
                line_passes(&heights, floor, cell, |cell, at| {
                    heights.walkable_on(cell, at)
                        && !crowd.stops_but(cell, at, (entity, false), aimed)
                })
            })
        };
        let stares = opensacred_rules::casts::stare(mind.creature).is_some();
        let demon = opensacred_rules::casts::big_demon(mind.creature);
        // A magician's friends within its reach of them, the first wounded and the first that
        // fights (#1854).
        let near: Vec<&(Entity, Vec3, bool, bool)> = friends
            .iter()
            .filter(|(friend, at, ..)| {
                *friend != entity
                    && (*at - here).xz().length() <= opensacred_rules::casts::FRIENDS_WITHIN
            })
            .collect();
        let wounded_friend = near
            .iter()
            .find(|(.., wounded, _)| *wounded)
            .map(|f| (f.0, f.1));
        let fighting_friend = near.iter().find(|(.., fights)| *fights).map(|f| (f.0, f.1));
        // A caster's decision first (#1852), while it casts none; the casts the game follows.
        // A boss's or a champion's keep-back too, whatever its mark (#2563).
        let elite = rank.is_some_and(|rank| rank.0 != crate::fight::Rank::Common);
        let cast =
            (!casting && (crate::creature_casts::casts(mind.creature) || elite)).then(|| {
                let sight_of_cast = opensacred_rules::minds::CastSight {
                    now,
                    at_hero: foe.is_none(),
                    hero_level,
                    held: hero_held,
                    // The hero of set 52's six pieces, whose holds may be turned (#3437).
                    uncaged: foe.is_none()
                        && holding.is_some_and(|holding| holding.0.sets.turns_holds()),
                    // The hero with a bow or a crossbow in a hand (`0x54A2A0`), or the Mage (the
                    // type 3), by which a creature of the mark 4 takes its special the more
                    // (#3263); a creature fought, none.
                    ranged_target: foe.is_none()
                        && (holding.is_some_and(|holding| holding.0.ranged)
                            || kind.as_deref().is_some_and(|hero| hero.creature == MAGE)),
                    elite: rank.is_some_and(|rank| rank.0 != crate::fight::Rank::Common),
                    hostiles: hand_to_hand,
                    crowded: nearest.hostiles.len() > 1,
                    nearest: nearest.hostiles.first() == Some(&entity),
                    scripted: u16::from(mind.spell),
                    hero_low: mind.creature == opensacred_rules::casts::SHARIFA
                        && hero_health.is_some_and(|(current, most)| {
                            opensacred_rules::casts::hero_low(current, most)
                        }),
                    followers: summoned.get(&entity).copied().unwrap_or(0),
                    friends: (wounded_friend.is_some(), fighting_friend.is_some()),
                    id: stand_in_id(mind.home),
                    hornets: {
                        let others = || {
                            hornets.iter().filter(|&&(other, creature, _)| {
                                other != entity && creature == opensacred_rules::casts::HORNET
                            })
                        };
                        (
                            u32::try_from(others().count()).unwrap_or(u32::MAX),
                            others().any(|&(.., strikes)| strikes),
                        )
                    },
                    hornet_lives: hornets.iter().any(|&(other, ..)| other != entity),
                    listed: u32::try_from(nearest.hostiles.len()).unwrap_or(u32::MAX),
                    difficulty,
                    fetzer: fetzers
                        .iter()
                        .find(|&&other| other != entity)
                        .map(|&other| attacked == Some(other)),
                    // Whether its right hand holds an item, a minimummy's dagger (#2575).
                    armed: arms.get(entity).is_ok_and(|arms| arms.right().is_some()),
                    // A dryad of `0x20E` near its target, the point it may leap to (#2566).
                    leap: dryads_leap(
                        (entity, &mind),
                        (here, target, target_size),
                        (way.level, &heights, &crowd),
                        &mut minds.rng,
                    ),
                    group_strike: mind.group.and_then(|group| {
                        strikers
                            .iter()
                            .filter(|&&(other, of, _)| other != entity && of == group)
                            .nth(1)
                            .map(|&(.., size)| size)
                    }),
                    // A brain-footer's strength by its level on bronze's scale against the
                    // hero's (#2567).
                    stare: opensacred_rules::casts::stare(mind.creature)
                        .and(rules.as_deref())
                        .and_then(|rules| {
                            opensacred_rules::casts::stare_strength(
                                rules.on_bronze(mind.level),
                                hero_level,
                                rules.weighing_terms(),
                            )
                        }),
                };
                let line = || {
                    if stares {
                        sighted()
                    } else if demon {
                        walked()
                    } else {
                        clear()
                    }
                };
                mind.cast(&sight, &sight_of_cast, line, &mut minds.rng)
            });
        if casting {
            continue;
        }
        // Fighting the hero, far from home in another sector, it may go home (the leash, #2489).
        let home_elsewhere = sector_of(point(mind.home, here.y)) != sector_of(here);
        let leashed = foe.is_none()
            && !striking
            && mind.leash(
                spot(here),
                ((target - here).xz().length(), hero_level),
                home_elsewhere,
                sight.health,
            );
        let Some(pursues) = (if leashed {
            Some(Pursues::GoesBack)
        } else {
            cast.flatten()
                .or_else(|| mind.pursue(&sight, sighted, &mut minds.rng))
        }) else {
            continue;
        };
        if !mind.fighting() {
            mind.foe = None;
        }
        match pursues {
            Pursues::GivesUp => {
                let d = (hero - here).xz().length();
                debug!("ai: {entity} {:?} at {d:.0}", Why::GivesUp);
            }
            Pursues::StruckDown => {
                // It stops what it does (`0x54E6D0(1, 0)`) and takes the hero's sure strike, the
                // event 5 that it sends itself from the hero (`0x4E1157`).
                debug!("ai: {entity} would flee, struck down by the hero's set 58");
                commands.entity(entity).remove::<(Walk, Strikes)>();
                hits.write(crate::fight::Hit {
                    attacker: the_hero,
                    target: entity,
                    share: 1.0,
                    ranged: false,
                    art: None,
                    fixed: Some(minds::STRUCK_DOWN),
                    sure: true,
                    poison: false,
                    ward: false,
                    hurled: false,
                    object: false,
                    mount: false,
                    marks: true,
                });
            }
            Pursues::Flees(to) => {
                debug!("ai: {entity} {:?} the hero", Why::Flees);
                // A crow that took off flies its flight (`0x4E1336`, #2291).
                let walk = if mind.flying {
                    Walk::fly(point(to, here.y), way.level, mind.fly)
                } else {
                    Walk::run(point(to, here.y), way.level, mind.run)
                };
                commands.entity(entity).insert(walk);
            }
            Pursues::ComesOn { runs, within } => {
                // In melee without a straight line: a way to the hero, or the door in its way, or
                // it gives up (`0x4E4931`, `0x4F5652`).
                if !mind.ranged && !walked() {
                    let reaches = |to: Spot| way_to(here, way.level, to, &heights);
                    let mut found = None;
                    let door = || {
                        found = door_in_way(here, layered.layer, &seen);
                        found.is_some()
                    };
                    match mind.blocked(spot(hero), reaches, door, &mut minds.rng) {
                        Blocked::ComesOn => {}
                        Blocked::Door => {
                            if let Some(found) = found {
                                go_for_door(
                                    &mut commands,
                                    (entity, here, way, doors.rides(entity)),
                                    &mut mind,
                                    found,
                                    &heights,
                                );
                            }
                            continue;
                        }
                        Blocked::GivesUp => {
                            let (from, to) = (
                                paths::cell_of(here.x, here.z),
                                paths::cell_of(hero.x, hero.z),
                            );
                            debug!(
                                "ai: {entity} {:?}, no way from {from:?} on {:?} to the hero at \
                                 {to:?}",
                                Why::GivesUp,
                                way.level
                            );
                            continue;
                        }
                    }
                }
                commands
                    .entity(entity)
                    .insert(come_on(&mind, hero, way, runs, within));
            }
            Pursues::TakesPlace { to, runs, within } => {
                debug!(
                    "ai: {entity} takes its place about the hero at {:.0},{:.0}, running {runs}",
                    to.x, to.y
                );
                let to = point(to, here.y);
                let walk = if runs {
                    Walk::run(to, way.level, mind.run)
                } else {
                    Walk::to(to, way.level, mind.speed)
                };
                commands.entity(entity).insert(walk.within(within));
            }
            Pursues::Charges { to, again } => {
                debug!("ai: {entity} charges the hero, again {again}");
                commands
                    .entity(entity)
                    .insert(Walk::run(point(to, here.y), way.level, mind.run).within(0.0));
                // Again, with the effect 24 on its horse for 10 s: its floops, its trample
                // striking nothing, no player riding (`0x4E33D5`).
                if again {
                    commands.queue(move |world: &mut World| {
                        crate::trample::charge(world, entity);
                    });
                }
            }
            Pursues::Passes { to } => {
                debug!("ai: {entity} strikes the hero in passing");
                // A plain strike with no clip, and the 842's sparks at its target
                // (`0x4EA3F0`, the value 1002; **hypothesis**: the green hit sparks).
                commands.queue(move |world: &mut World| {
                    world.write_message(crate::fight::Hit {
                        attacker: entity,
                        target: aimed,
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
                        marks: false,
                    });
                    world.write_message(crate::hit_sparks::Spark {
                        art: crate::trample::TRAMPLE_SPARKS,
                        by: entity,
                        on: aimed,
                    });
                });
                commands
                    .entity(entity)
                    .insert(Walk::run(point(to, here.y), way.level, mind.run));
            }
            Pursues::Swoops { to, strikes } => {
                debug!(
                    "ai: {entity} swoops to {:.0},{:.0} from {:.0},{:.0}, {:.0} from {aimed},                      moving {}",
                    to.x,
                    to.y,
                    here.x,
                    here.z,
                    (hero - here).xz().length(),
                    walk.is_some()
                );
                // Its pass strikes its target once with no clip as it flies on beyond it
                // (`0x4EA3F0` at the sub-state 7, #3589); its flight at the gait 2, aloft.
                if strikes {
                    debug!("ai: {entity} swoops through {aimed}");
                    passes.write(SwoopPassed {
                        flyer: entity,
                        target: aimed,
                    });
                    hits.write(crate::fight::Hit {
                        attacker: entity,
                        target: aimed,
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
                        marks: false,
                    });
                }
                commands
                    .entity(entity)
                    .remove::<Strikes>()
                    .insert(Walk::fly(point(to, here.y), way.level, mind.fly));
            }
            Pursues::RidesAway { to } => {
                commands
                    .entity(entity)
                    .insert(Walk::run(point(to, here.y), way.level, mind.run));
            }
            Pursues::Circles { to } => {
                debug!(
                    "ai: {entity} rides round the hero to {:.0},{:.0}",
                    to.x, to.y
                );
                commands
                    .entity(entity)
                    .insert(Walk::run(point(to, here.y), way.level, mind.run));
            }
            // At a run in two cases of three (#2489).
            Pursues::StepsBack(to) => {
                let to = point(to, here.y);
                // A step back from the hero's own point is a move to where it stands, which ends
                // at once (`0x4E20EE`): it stands, and strikes as it decides next (#2513).
                if (to - here).xz().length() < crate::hero::ARRIVED {
                    debug!("ai: {entity} stands on the hero's point and steps nowhere");
                    commands.entity(entity).remove::<Walk>();
                } else {
                    commands
                        .entity(entity)
                        .insert(if mind.steps_back_runs(&mut minds.rng) {
                            Walk::run(to, way.level, mind.run)
                        } else {
                            Walk::to(to, way.level, mind.speed)
                        });
                }
            }
            Pursues::GoesBack => {
                debug!("ai: {entity} {:?} from the fight", Why::GoesBack);
                let home = point(mind.home, here.y);
                commands
                    .entity(entity)
                    .insert(Walk::to(home, way.level, mind.speed));
            }
            // In the band, turned to the hero, it strikes, its walk to its place ended.
            Pursues::Strikes(choice) => {
                transform.rotation = crate::hero::facing(hero - here);
                if let Some(choice) = choice {
                    let chains = std::mem::take(&mut mind.chains);
                    // A strike that does not chain, against a target that fights (the hero
                    // attacking, a creature fighting), is a parry in one case of four where its
                    // model has its defending clip: it plays the clip once in place of the strike,
                    // neither striking nor speaking (`0x4E4ECF`–`0x4E4F85`, #2487).
                    let target_fights = match foe {
                        Some(foe) => foes.get(&foe).is_some_and(|&(.., fighting, _)| fighting),
                        None => attacking,
                    };
                    if !chains
                        && target_fights
                        && minds.rng.below(4) == 1
                        && let Some(slot) = clips.get(entity).ok().and_then(|npc| npc.defend_slot())
                    {
                        debug!("ai: {entity} parries {aimed} by its slot {slot}");
                        commands
                            .entity(entity)
                            .remove::<Walk>()
                            .insert(crate::npcs::ScriptClip::parry(slot));
                        continue;
                    }
                    // A strike that does not chain speaks (`0x4E52D1`, #711).
                    if !chains {
                        lines.write(crate::voices::Speak {
                            speaker: entity,
                            group: crate::voices::ATTACK,
                        });
                    }
                    commands
                        .entity(entity)
                        .remove::<Walk>()
                        .insert(Strikes::at(aimed, choice).opening(!chains));
                }
            }
            Pursues::BacksOff { to, runs } => {
                let d = (hero - here).xz().length();
                debug!("ai: {entity} backs off from the hero at {d:.0}, running {runs}");
                let to = point(to, here.y);
                commands.entity(entity).insert(if runs {
                    Walk::run(to, way.level, mind.run)
                } else {
                    Walk::to(to, way.level, mind.speed)
                });
            }
            Pursues::GoesRound { to, runs, within } => {
                debug!("ai: {entity} goes round the hero, no straight line to it");
                let to = point(to, here.y);
                let walk = if runs {
                    Walk::run(to, way.level, mind.run)
                } else {
                    Walk::to(to, way.level, mind.speed)
                };
                commands.entity(entity).insert(walk.within(within));
            }
            // It stops where it stands, turns to the hero and shoots.
            Pursues::Shoots { choice, chain } => {
                let d = (hero - here).xz().length();
                debug!("ai: {entity} shoots at {aimed} from {d:.0}, its chain {chain:.1}");
                transform.rotation = crate::hero::facing(hero - here);
                // A shot speaks (`0x4DF06C`, #711).
                lines.write(crate::voices::Speak {
                    speaker: entity,
                    group: crate::voices::ATTACK,
                });
                // Its bolts under the Seraphim's effect 16, else 17, in place of its arrow
                // (`0x4DCB80`, #2899): the clip the effect 16's where it holds it, the bolts the
                // effect 17's where it holds it, else the effect 16's while its staff is in its
                // hand, else none (`0x4DCD70`, `0x4DD92F`, `0x4DE38C`; #3030).
                let holds = |effect| effects.is_some_and(|effects| effects.holds(effect));
                let (bfg, railgun) = (
                    holds(opensacred_rules::arts::BFG),
                    holds(opensacred_rules::arts::RAILGUN),
                );
                let bolt = opensacred_rules::arts::Bolt::of(bfg, railgun, None);
                let staff = hands.is_some_and(|hands| hands.staff);
                let fires = if railgun {
                    Some(opensacred_rules::arts::Bolt::Railgun)
                } else {
                    (bfg && staff).then_some(opensacred_rules::arts::Bolt::Bfg)
                };
                commands.entity(entity).remove::<Walk>().insert(
                    Strikes::shot(aimed, choice, chain)
                        .bolts(bolt)
                        .firing(fires),
                );
            }
            // It stops where it stands and changes its weapons (`npcs::rearm`).
            Pursues::ToMelee(weapon) => {
                let d = (hero - here).xz().length();
                debug!(
                    "ai: {entity} puts its bow away for melee, the hero at {d:.0}, taking \
                     {weapon:?}, to take a bow again {}",
                    mind.takes_bow()
                );
                commands
                    .entity(entity)
                    .remove::<Walk>()
                    .insert(Rearms::to_melee(weapon));
            }
            // The terror sprinter mage's aura while its key holds it, refused (`0x51D840`).
            // On itself, a spell refused while its key holds the caster (`0x51D840`); the stone
            // and the cage at a hero above their bound of levels (`0x51D90D`, #3239); one on an
            // enemy, on the ground or on the dead at a target on another level than its own
            // (`0x51D840` at `0x449930`, docs/re/fight.md, "Across a building's levels", #3852).
            Pursues::Casts { spell, .. }
                if !crate::creature_casts::follows(spell)
                    || (target_level != way.level
                        && arts.as_deref().is_some_and(|arts| {
                            arts.tables
                                .spells
                                .iter()
                                .find(|known| known.id == spell)
                                .is_some_and(|known| levels_tested(known.launch))
                        }))
                    || ((spell == opensacred_rules::casts::SPRINTERS_AURA
                        || opensacred_rules::casts::on_itself(spell))
                        && opensacred_rules::casts::refused_while_keyed(spell)
                        && effects.is_some_and(|effects| effects.holds_any(&[spell])))
                    || rules.as_deref().is_some_and(|rules| {
                        let rank = rank.map_or(crate::fight::Rank::Common, |rank| rank.0);
                        let level = rules.creature_spell_level(mind.level, rank);
                        !opensacred_rules::spells::level_admits(
                            spell,
                            (hero_level, false),
                            (mind.level, level),
                        )
                    }) =>
            {
                // A spell refused, or one the game does not follow yet: it stands facing its
                // target.
                debug!("ai: {entity} would cast {spell}, refused or not followed");
                transform.rotation = crate::hero::facing(hero - here);
            }
            Pursues::Casts { spell, variant } => {
                // A heal or a shield on its friend (#1854), else at its target; Sharifa's heal on
                // the friend of the least share of health within 700, herself first (#1883).
                let friend = if spell == opensacred_rules::casts::HEAL
                    && mind.creature == opensacred_rules::casts::SHARIFA
                {
                    let own = health.map(|health| (entity, here, health.current, health.most));
                    let candidates: Vec<((Entity, Vec3), u32, u32)> = own
                        .into_iter()
                        .chain(allies.iter().copied().filter(|&(ally, at, ..)| {
                            ally != entity
                                && (at - here).xz().length()
                                    < opensacred_rules::casts::FRIENDS_WITHIN
                        }))
                        .map(|(who, at, current, most)| ((who, at), current, most))
                        .collect();
                    opensacred_rules::casts::least_share(&candidates)
                } else if spell == opensacred_rules::casts::HEAL {
                    wounded_friend
                } else {
                    fighting_friend
                };
                let (aimed, hero) = if opensacred_rules::casts::on_a_friend(spell) {
                    friend.unwrap_or((entity, here))
                } else if opensacred_rules::casts::on_itself(spell) {
                    (entity, here)
                } else if opensacred_rules::casts::mark_8(mind.creature)
                    && hero_followers.contains(&aimed)
                    && opensacred_rules::casts::at_the_hero((heros_place - here).xz().length())
                {
                    // A follower of the hero within 750 of the hero: at the hero (`0x4D6939`).
                    (the_hero, heros_place)
                } else {
                    (aimed, hero)
                };
                // A friend on another level the launch test refuses, the caster itself passing
                // (`0x51D840`; docs/re/fight.md, "A friend on another level", #3857): it casts
                // nothing this time, and may choose the friend again at its next decision.
                let friend_level = if aimed == entity {
                    way.level
                } else if aimed == the_hero {
                    hero_layer
                } else {
                    foes.get(&aimed).map_or(way.level, |&(.., level)| level)
                };
                transform.rotation = crate::hero::facing(hero - here);
                if opensacred_rules::casts::on_a_friend(spell) && friend_level != way.level {
                    debug!("ai: {entity} would cast {spell} at {aimed} on another level, refused");
                } else {
                    let d = (hero - here).xz().length();
                    debug!("ai: {entity} casts {spell} at {aimed} from {d:.0}");
                    commands.entity(entity).remove::<Walk>().insert(
                        crate::creature_casts::Casting::new(spell, variant, aimed, hero),
                    );
                }
            }
            // A hold that the hero's set 52 turns (#3437): cast toward the hero still, at the
            // caster itself or at the hero's nearest hostile; with none listed it casts nothing
            // (**hypothesis**: the launch test refuses an order at no creature), and no level's
            // test (**hypothesis**: the launch test lets the caster through).
            Pursues::HoldsTurned { spell, onto } => {
                transform.rotation = crate::hero::facing(hero - here);
                let aimed = match onto {
                    opensacred_rules::casts::Turned::Caster => Some(entity),
                    opensacred_rules::casts::Turned::Nearest => nearest.hostiles.first().copied(),
                };
                if let Some(aimed) = aimed {
                    debug!("ai: {entity} casts {spell} toward the hero, turned onto {aimed}");
                    commands
                        .entity(entity)
                        .remove::<Walk>()
                        .insert(crate::creature_casts::Casting::new(spell, 0, aimed, hero));
                } else {
                    debug!("ai: {entity}'s {spell} turned onto no hostile of the hero");
                }
            }
            Pursues::Faces => {
                if walk.is_none() {
                    transform.rotation = crate::hero::facing(hero - here);
                }
            }
            // At a run near its target, else as it comes on (#2489).
            Pursues::KeepsBand(to) => {
                let d = (hero - here).xz().length();
                let to = point(to, here.y);
                commands
                    .entity(entity)
                    .insert(if mind.keeps_band_runs(d, &mut minds.rng) {
                        Walk::run(to, way.level, mind.run)
                    } else {
                        Walk::to(to, way.level, mind.speed)
                    });
            }
            Pursues::Strolls(to, runs) => {
                let to = point(to, here.y);
                let walk = if runs {
                    Walk::run(to, way.level, mind.run)
                } else {
                    Walk::to(to, way.level, mind.speed)
                };
                commands.entity(entity).insert(walk.within(STROLL_WITHIN));
            }
            Pursues::BlinksTo(to) => {
                // Where it may stand, with a battle fog where it leaves and where it lands
                // (`0x54D9D0`, `0x4D8F49`); else nothing.
                let at = point(to, here.y);
                if let Some(height) = heights.standing(at.x, at.z, way.level) {
                    let to = Vec3::new(at.x, height, at.z);
                    let seconds = opensacred_rules::casts::blink_fog_seconds(mind.level);
                    debug!("ai: {entity} blinks from {here} to {to}");
                    crate::battle_fogs::rise(
                        &mut commands,
                        here,
                        seconds,
                        crate::battle_fogs::Variant::Leaves,
                    );
                    crate::battle_fogs::rise(
                        &mut commands,
                        to,
                        seconds,
                        crate::battle_fogs::Variant::Lands,
                    );
                    transform.translation = to;
                    commands.entity(entity).remove::<Walk>();
                }
            }
            Pursues::Leaps(to) => {
                let to = point(to, here.y);
                debug!("ai: {entity} leaps to {to}");
                commands
                    .entity(entity)
                    .remove::<(Walk, Strikes)>()
                    .insert(crate::creature_arts::Leaps::start(here, to));
            }
            Pursues::TakesDagger => {
                debug!("ai: {entity} takes a dagger");
                commands
                    .entity(entity)
                    .insert(crate::npcs::Rearms::dagger());
            }
            Pursues::ThrowsDaggers { more } => {
                debug!("ai: {entity} throws its dagger at {aimed} and {more} more");
                transform.rotation = crate::hero::facing(target - here);
                commands
                    .entity(entity)
                    .remove::<(Walk, Strikes)>()
                    .insert(crate::creature_arts::Volley::new(aimed, more));
            }
            Pursues::Stares { strength } => {
                debug!("ai: {entity} stares at {aimed} of the strength {strength:.2}");
                commands.entity(entity).remove::<(Walk, Strikes)>();
                let creature = mind.creature;
                let roll = minds.rng.below(opensacred_rules::casts::STARE_BONES);
                commands.queue(move |world: &mut World| {
                    crate::creature_arts::stare(world, (entity, aimed), (creature, strength), roll);
                });
            }
            // A weapon art on an enemy, a friend, a point or the dead at a target on another
            // level the launch test refuses, its creature standing; one on itself is made
            // (`0x51D840`, #3857).
            Pursues::MakesArt { art }
                if target_level != way.level
                    && arts.as_deref().is_some_and(|arts| {
                        arts.tables
                            .weapon_arts
                            .iter()
                            .find(|known| known.id == art)
                            .is_some_and(|known| levels_tested(u16::from(known.launch)))
                    }) =>
            {
                debug!(
                    "ai: {entity} would make the art {art} at {aimed} on another level, refused"
                );
            }
            Pursues::MakesArt { art } => {
                debug!("ai: {entity} is ordered the art {art} at {aimed}");
                commands
                    .entity(entity)
                    .remove::<Walk>()
                    .insert(crate::creature_arts::Ordered { art, target: aimed });
            }
            Pursues::Hides { from } => {
                // The first of its points on the ground that it may stand on, out of the hero's
                // straight line, with a path to it (`0x4D98A4`); with none it decides again.
                let found =
                    opensacred_rules::minds::hiding_points(spot(here), from).find_map(|to| {
                        let at = point(to, here.y);
                        let height = heights.standing(at.x, at.z, None)?;
                        // The creatures on the line stop it but its target (`0x4D9995`, #3360).
                        let seen = paths::straight(hero.xz(), at.xz(), |cell| {
                            heights.walkable_on(cell, None)
                                && !crowd.stops_but(cell, None, (entity, false), aimed)
                        });
                        (!seen && way_to(here, way.level, to, &heights))
                            .then_some(Vec3::new(at.x, height, at.z))
                    });
                match found {
                    Some(to) => {
                        debug!("ai: {entity} hides from the hero at {to}");
                        commands
                            .entity(entity)
                            .insert(Walk::run(to, None, mind.run).within(STROLL_WITHIN));
                    }
                    None => mind.hides_nowhere(),
                }
            }
            Pursues::Blinks => {
                // The first of its tries about it that a straight way reaches, each within ±500
                // along each axis (`0x4D722B`).
                let level = way.level;
                let found = (0..opensacred_rules::casts::BLINK_TRIES).find_map(|_| {
                    let (x, y) = (minds.rng.below(0x8000), minds.rng.below(0x8000));
                    let (dx, dz) = opensacred_rules::casts::blink_try(x, y);
                    let to = here.xz() + Vec2::new(dx, dz);
                    // The creatures on the line stop it but the hero (`0x4D72C3`, #3360).
                    let reaches = paths::straight(here.xz(), to, |cell| {
                        heights.walkable_on(cell, level)
                            && !crowd.stops_but(cell, level, (entity, false), the_hero)
                    });
                    let height = heights.standing(to.x, to.y, level)?;
                    reaches.then_some(Vec3::new(to.x, height, to.y))
                });
                if let Some(to) = found {
                    debug!("ai: {entity} blinks from {here} to {to}");
                    // The pale green sparks of its figure where it stood (`0x4D72E8`, #2898).
                    commands.queue(move |world: &mut World| {
                        crate::group_looks::flash(
                            world,
                            entity,
                            opensacred_rules::casts::BLINK_COLOUR,
                        );
                    });
                    transform.translation = to;
                    commands.entity(entity).remove::<Walk>();
                }
            }
            Pursues::DruidBlinks { to, level } => {
                // The blink where it may stand there (`0x54D9D0`), else nothing.
                let at = point(to, here.y);
                let Some(height) = heights.standing(at.x, at.z, way.level) else {
                    continue;
                };
                let to = Vec3::new(at.x, height, at.z);
                debug!("ai: {entity} blinks from {here} to {to} and sets thorns at {hero}");
                transform.translation = to;
                transform.rotation = crate::hero::facing(hero - to);
                commands.entity(entity).remove::<Walk>().insert(
                    crate::creature_casts::DruidThorns {
                        from: here,
                        to,
                        target: hero,
                        level,
                    },
                );
            }
            Pursues::TakesBow(bow) => {
                let d = (hero - here).xz().length();
                debug!("ai: {entity} takes the bow {bow} again, the hero at {d:.0}");
                commands
                    .entity(entity)
                    .remove::<Walk>()
                    .insert(Rearms::bow(bow));
            }
        }
    }
}

/// The point a dryad of `0x20E` near its target at `target` may leap to (`0x4D86BC`, #2566), none
/// for another creature or where none fits: one it may stand on, to which its straight way runs
/// clear of all but the creatures (the order's test, `0x4FD0F0`, its flag 1), but whose straight
/// way back to it some creature blocks (`0x4FCC80`, its flag 0; the target and itself aside),
/// and from which a way leads to its target (`0x4F7710`).
fn dryads_leap(
    (entity, mind): (Entity, &Mind),
    (here, target, target_size): (Vec3, Vec3, u32),
    (level, heights, crowd): (Level, &Heights, &Crowd),
    rng: &mut Rng,
) -> Option<Spot> {
    let reach = minds::reach_f32(mind.size(), target_size).floor();
    if !matches!(mind.creature, 218 | 219)
        || mind.mark() == opensacred_rules::casts::DRYAD_LANDED
        || (target - here).xz().length().floor() >= 2.0 * reach
    {
        return None;
    }
    let target_cell = paths::cell_of(target.x, target.z);
    let open = |cell| heights.walkable_on(cell, level);
    // Its leap's line flown over the water and the fences, its way back walked (`0x4D87E8`,
    // #3358).
    let flown = |cell| heights.flown_onto(cell, level);
    opensacred_rules::casts::leap_point(spot(here), rng, |to| {
        let at = Vec2::new(to.x, to.y);
        heights.standing(at.x, at.y, level).is_some()
            && paths::straight(here.xz(), at, flown)
            && !paths::straight(at, here.xz(), |cell| {
                open(cell) && (cell == target_cell || !crowd.stops(cell, level, entity, false))
            })
            && way_to(point(to, here.y), level, spot(target), heights)
    })
}

/// A walk or a run at the hero at `hero`, stopping within `within` of it, in the band.
fn come_on(mind: &Mind, hero: Vec3, way: &InTheWay, runs: bool, within: f32) -> Walk {
    let walk = if runs {
        Walk::run(hero, way.level, mind.run)
    } else {
        Walk::to(hero, way.level, mind.speed)
    };
    walk.within(within).coming_on()
}

/// The world point at the centre of global cell `cell`, on the ground plane.
fn centre((x, y): (i32, i32)) -> Option<Vec3> {
    let (x, y) = (
        f32::from(i16::try_from(x).ok()?),
        f32::from(i16::try_from(y).ok()?),
    );
    Some(crate::iso::ground(x + 0.5, y + 0.5))
}

/// A whole number at random from `low` to `high`, both included.
fn among(low: i32, high: i32, rng: &mut Rng) -> i32 {
    let span = u32::try_from(high - low + 1).unwrap_or(1);
    low + i32::try_from(rng.below(span)).unwrap_or(0)
}

/// How near its door a townsperson going in opens it and walks through (`0x4F0F5E`).
const OPENS_WITHIN: f32 = 350.0;

/// How near its point a caster's run about its target ends (`0x4D51ED`, #1852).
const STROLL_WITHIN: f32 = 20.0;

/// The way a townsperson at `here` on the ground goes into a house by the door `door` at `at`
/// with its `points` (the sub-states 8, 9 and 7, docs/re/ai.md, "Houses and doors", #431):
/// farther than 350 from the door, to its outer point, from which it goes on ([`Entering`]);
/// within 350, through the door to its inner point on the building's floor, opening it. `None`
/// where the door has no inner point on a floor or no outer one on the ground.
fn entering(
    (door, at, points): (Entity, Vec3, Option<DoorPoints>),
    here: Vec3,
    heights: &Heights,
) -> Option<Entering> {
    let points = points?;
    let ((inner, true), (outer, false)) = (points.inner?, points.outer?) else {
        return None;
    };
    let inner = centre(inner)?;
    let level = heights.level_of(paths::cell_of(inner.x, inner.z));
    let inner = inner.with_y(heights.standing(inner.x, inner.z, level).unwrap_or(here.y));
    if (at - here).xz().length() > OPENS_WITHIN {
        Some(Entering::ToDoor(
            centre(outer)?.with_y(here.y),
            (door, at, inner),
        ))
    } else {
        Some(Entering::Through(door, inner))
    }
}

/// How near a door's point a townsperson whose roll stops walks off (`0x4F332F`).
const BY_DOOR_POINT: f32 = 24.0;

/// Whether a townsperson at `here` stands within [`BY_DOOR_POINT`] of a point of the door
/// `door`, its inner or its outer one (`0x4F332F`).
fn by_door_point(door: Option<(Entity, Vec3, Option<DoorPoints>)>, here: Vec3) -> bool {
    door.and_then(|(_, _, points)| points)
        .is_some_and(|points| {
            [points.inner, points.outer]
                .into_iter()
                .flatten()
                .filter_map(|(cell, _)| centre(cell))
                .any(|point| (point - here).xz().length() <= BY_DOOR_POINT)
        })
}

/// A stage of a townsperson's going in ([`entering`]).
enum Entering {
    /// To the door's outer point, the door, where it stands and its inner point to go on to.
    ToDoor(Vec3, (Entity, Vec3, Vec3)),
    /// Through the door to its inner point.
    Through(Entity, Vec3),
}

/// The cell a townsperson goes in to by the door at `door` (`0x4EEFA0`): of 15 cells 2 to 9
/// cells from the door's along each axis, either way, that a building's ground floor holds and
/// that one may stand on, the farthest from the door; and its level.
fn indoors(door: Vec3, heights: &Heights, rng: &mut Rng) -> Option<(Vec3, crate::terrain::Level)> {
    let (dx, dy) = paths::cell_of(door.x, door.z);
    (0..DOOR_TRIES)
        .filter_map(|_| {
            let mut offset = || {
                let d = among(INDOORS.0, INDOORS.1, rng);
                if rng.below(2) == 0 { -d } else { d }
            };
            let cell = (dx + offset(), dy + offset());
            let level = heights.level_of(cell)?;
            heights
                .walkable_on(cell, Some(level))
                .then_some((centre(cell)?, Some(level)))
        })
        .max_by(|a, b| {
            let (da, db) = ((a.0 - door).length_squared(), (b.0 - door).length_squared());
            da.total_cmp(&db)
        })
}

/// The cell a townsperson goes out to by the door at `door` (`0x4F1AB9`): the first of 15 cells
/// -4 to 5 cells from the door's along each axis that no building covers and that one may walk
/// on.
fn outdoors(door: Vec3, heights: &Heights, rng: &mut Rng) -> Option<Vec3> {
    let (dx, dy) = paths::cell_of(door.x, door.z);
    (0..DOOR_TRIES).find_map(|_| {
        let cell = (
            dx + among(OUTDOORS.0, OUTDOORS.1, rng),
            dy + among(OUTDOORS.0, OUTDOORS.1, rng),
        );
        let at = centre(cell)?;
        (heights.building(at.x, at.z).is_none() && heights.walkable_on(cell, None)).then_some(at)
    })
}

/// Where a townsperson at `here` in danger runs from its threat, the nearest of `hostiles` within
/// 800 (`0x4F0BDA`), as the rules send it (`opensacred_rules::minds::runs_from`); `None` out of
/// danger.
fn danger(here: Vec3, hostiles: &[Vec3], rng: &mut Rng) -> Option<Vec3> {
    let threat = hostiles
        .iter()
        .copied()
        .filter(|at| (*at - here).xz().length() < DANGER)
        .min_by(|a, b| {
            (*a - here)
                .length_squared()
                .total_cmp(&(*b - here).length_squared())
        })?;
    Some(point(runs_from(spot(here), spot(threat), rng), here.y))
}

/// Sends the creatures not hostile that walk under an order of their own within 91 of the hero,
/// at its look, to a point within 200 of themselves that they reach, the first of 10 tries
/// (`opensacred_rules::minds::Mind::steps_aside`, `0x547070`): only while the hero's AI is in its
/// state 0, not attacking, and only a creature whose AI is not, whose walk is not one of the AI's
/// state 0 (`Walk::is_idle`). A script's walk, the original's `NPC_Goto`, stops at the hero
/// instead, whom the step's test does not let it pass (#977). The original's look stops such a
/// creature and uses its point for nothing (docs/decisions/ai.md, "Creature walking by the
/// standing hero steps aside").
fn aside(
    mut commands: Commands,
    mut minds: ResMut<Minds>,
    nearest: Res<Nearest>,
    heights: Res<Heights>,
    crowd: Res<Crowd>,
    hero: Query<(&Transform, Has<crate::attack::Attack>), With<Body>>,
    creatures: Query<(Entity, &Transform, &InTheWay, &Mind, &Walk), Without<Body>>,
) {
    let (Some((hero, false)), true) = (hero.iter().next(), nearest.looked) else {
        return;
    };
    let hero = hero.translation;
    for (entity, transform, way, mind, walk) in &creatures {
        if walk.is_idle() {
            continue;
        }
        let here = transform.translation;
        let from = (paths::cell_of(here.x, here.z), (way.level, false));
        let reaches = |to: Spot| {
            paths::path_onto(
                from,
                paths::cell_of(to.x, to.y),
                |_| true,
                |place| {
                    let mut next = heights.neighbours(place);
                    next.retain(|&(cell, (at, _))| !crowd.stops(cell, at, entity, false));
                    next
                },
            )
            .is_some()
        };
        let Some(target) = mind.steps_aside(spot(here), spot(hero), &mut minds.rng, reaches) else {
            continue;
        };
        let target = point(target, here.y);
        debug!(
            "ai: {entity} {:?} from {:.0},{:.0} to {:.0},{:.0}",
            Why::StepsAside,
            here.x,
            here.z,
            target.x,
            target.z
        );
        commands
            .entity(entity)
            .insert(Walk::to(target, way.level, mind.speed));
    }
}

/// The peaceful creatures' behaviours while they stand awake, at every tick (`0x4EF150`); a
/// creature farther than 1,500 from the hero goes dormant again, a townsperson farther than 600
/// times the camera's zoom. A townsperson decides at each of the hero's looks, which wake it, as
/// the original's goes dormant after most of its rolls and decides again when woken. An idle
/// creature, or a townsperson between its rolls, that shares its cell with another parts from it
/// by chance (`0x544890`); an animal awake strolls at once. The decisions are the rules'
/// (`opensacred_rules::minds`); this system finds the creatures, the doors and the cells they
/// take, and walks, turns and stops them.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn behave(
    mut commands: Commands,
    mut minds: ResMut<Minds>,
    nearest: Res<Nearest>,
    crowd: Res<Crowd>,
    (heights, dungeons): (Res<Heights>, Option<Res<crate::daynight::Dungeons>>),
    (hero, keys): (
        Query<(Entity, &Transform, &Body, Option<&crate::attack::Attack>)>,
        crate::controls::Keys,
    ),
    cameras: Query<&Projection, crate::iso::Eye>,
    doors: Query<(Entity, &Object, &Door), Without<Mind>>,
    npcs: Option<Res<Npcs>>,
    clock: Res<Clock>,
    daemon: Res<Daemon>,
    mut creatures: Query<
        (Entity, &mut Transform, &InTheWay, &mut Mind, Option<&Walk>),
        (Without<Body>, Without<crate::npcs::Dies>),
    >,
    forms: Option<Res<crate::forms::Forms>>,
    (followers, queued, arms, clips): (
        Query<(), With<crate::followers::Follower>>,
        Query<
            (),
            Or<(
                With<crate::npcs::ScriptQueue>,
                With<crate::npcs::ScriptClip>,
            )>,
        >,
        Query<&crate::npcs::Arms>,
        Query<&crate::npcs::Npc>,
    ),
) {
    let Some((the_hero, hero, body, attack)) = hero.iter().next() else {
        return;
    };
    // The hero shoos the children standing with Shift held, the walk's key (`0x4F2AD7`, #1364).
    let shoos = body.moving().is_none() && keys.pressed(crate::controls::Action::Walk);
    // The hero attacks while its attack has a target (`0x4EB890`).
    let hero_attacks = attack.is_some_and(|attack| attack.target().is_some());
    let hero = hero.translation;
    // Every creature as a guard sees it, for the nearest hostile to it, the hero among them
    // (`0x4EEE40`): the hero of its side 1 and a type of no exception's.
    let others: Vec<Other> = creatures
        .iter()
        .map(|(entity, transform, _, mind, _)| Other::of(entity, transform.translation, mind))
        .chain(std::iter::once(Other {
            entity: the_hero,
            at: hero,
            side: minds::HEROES,
            creature: 0,
            class: 0,
            level: 0,
            size: minds::SIZE,
            turned: false,
        }))
        .collect();
    // Whether the hero stands on a settlement's ground, where the children gather about it.
    let in_settlement = heights
        .flags(paths::cell_of(hero.x, hero.z))
        .is_some_and(|flags| flags & SETTLEMENT != 0);
    // The children at play this tick, whom the others follow.
    let mut playing: Vec<(Entity, Vec3)> = Vec::new();
    // The camera's zoom, its scale.
    let zoom = cameras
        .iter()
        .find_map(|projection| match projection {
            Projection::Orthographic(ortho) => Some(ortho.scale),
            _ => None,
        })
        .unwrap_or(1.0);
    let side = crate::iso::CELL * 64.0;
    let sector = |p: Vec3| ((p.x / side).floor(), (p.z / side).floor());
    // What the animals may fear: every creature, an animal of the classes 4 and 6 only while it
    // attacks, and the hero (`minds::Mind::fears`, #1364).
    let feared: Vec<(Entity, Vec3, bool, u32, bool)> = creatures
        .iter()
        .map(|(entity, transform, _, mind, _)| {
            (
                entity,
                transform.translation,
                matches!(mind.class, 4 | 6),
                mind.flags,
                mind.fighting(),
            )
        })
        .chain(std::iter::once((
            Entity::PLACEHOLDER,
            hero,
            false,
            0,
            hero_attacks,
        )))
        .collect();
    // What the townsfolk visit and take for partners: the farm and wild animals, the humans of
    // class 3 and the traders, at the hero's looks.
    let [animals, humans, traders]: [Vec<(Entity, Vec3)>; 3] = if nearest.looked {
        let of = |wanted: fn(&Mind) -> bool| -> Vec<(Entity, Vec3)> {
            creatures
                .iter()
                .filter(|(_, _, _, mind, _)| wanted(mind))
                .map(|(entity, transform, ..)| (entity, transform.translation))
                .collect()
        };
        [
            of(|mind| mind.class == 6 && mind.is_animal()),
            of(|mind| mind.class == 3),
            of(|mind| mind.trader),
        ]
    } else {
        [Vec::new(), Vec::new(), Vec::new()]
    };
    // The partners' places, whether they walk, and whether they are traders.
    let followed: std::collections::HashMap<Entity, (Vec3, bool, bool)> = creatures
        .iter()
        .filter_map(|(_, _, _, mind, _)| mind.partner)
        .filter_map(|partner| {
            let (_, transform, _, mind, walk) = creatures.get(partner).ok()?;
            Some((
                partner,
                (transform.translation, walk.is_some(), mind.trader),
            ))
        })
        .collect();
    // The hero's hostiles, which put the townsfolk near them in danger: those its last look
    // took (`0xAD3CE0`, `crowd::look`).
    let hostiles: Vec<Vec3> = nearest
        .hostiles
        .iter()
        .filter_map(|&hostile| creatures.get(hostile).ok())
        .map(|(_, transform, ..)| transform.translation)
        .collect();
    // The doors at their logical positions, with their points, at the hero's looks.
    let doors: Vec<(Entity, Vec3, Option<DoorPoints>)> = if nearest.looked {
        doors
            .iter()
            .map(|(entity, object, door)| {
                let points = npcs
                    .as_ref()
                    .and_then(|npcs| door_points(door, npcs.door_cell(door), &heights));
                (entity, object.at, points)
            })
            .collect()
    } else {
        Vec::new()
    };
    // The doors to open and to close this tick.
    let mut swings: Vec<(Entity, bool)> = Vec::new();
    // At dusk and by night the townsfolk take the evening's way (`0x41BB50`, `0x41BAB0`).
    let evening = matches!(clock.phase(), Phase::Dusk | Phase::Night);
    // By night, not at dusk, the guards and the townsfolk take torches (`0x41BAB0`).
    let night = clock.phase() == Phase::Night;
    let mut walks = Vec::new();
    let mut turns: Vec<(Entity, Vec3)> = Vec::new();
    // The last child shooed this tick, which the hero faces (#3150).
    let mut shooed: Option<Vec3> = None;
    let mut stops: Vec<Entity> = Vec::new();
    // The doings written on others than the one deciding: a meeting's partner's (`0x4FC650`).
    let mut doings: Vec<(Entity, u16)> = Vec::new();
    let mut torches: Vec<(Entity, minds::Torch)> = Vec::new();
    for (entity, transform, way, mut mind, walk) in &mut creatures {
        // A follower keeps by its master instead (`followers`), and one whose script's queue runs
        // by its queue (`0x544890` at `0x545B1B`, #1220).
        if followers.contains(entity) || queued.contains(entity) {
            continue;
        }
        let here = transform.translation;
        let left = || arms.get(entity).ok().and_then(crate::npcs::Arms::left);
        // On the ground outside a dungeon a flyer takes off (`+0x24` 0, `0x63D180`, #2250).
        let in_the_open = way.level.is_none()
            && !dungeons
                .as_deref()
                .is_some_and(|dungeons| dungeons.holds(here.x, here.z));
        // A guard awake within 1,500 of the hero takes a torch into its empty left hand by night
        // and puts it away by day (`0x4EFA50`), before it looks for a foe.
        if mind.active
            && mind.is_guard()
            && (here - hero).xz().length() <= BEHAVES_WITHIN
            && let Some(torch) = mind.guard_torch(odd(mind.home), night, left())
        {
            torches.push((entity, torch));
        }
        // A guard awake, walking or standing, attacks the nearest creature of the 3 × 3 sectors
        // about it that it is hostile to, within 500 (`0x4EF150`, `0x4EEE40`, the event `0x100`).
        if mind.active
            && mind.is_guard()
            && (here - hero).xz().length() <= BEHAVES_WITHIN
            && let Some(foe) = nearest_foe(entity, here, &mind, &others)
            && mind.guards(spot(here), spot(foe.at))
        {
            let d = (foe.at - here).xz().length();
            debug!(
                "ai: {entity} {:?} {}, a {} of the side {}, from {d:.0}",
                Why::Attacks,
                foe.entity,
                foe.creature,
                foe.side
            );
            mind.foe = (foe.entity != the_hero).then_some(foe.entity);
            mind.activity = activity::FIGHTS;
            stops.push(entity);
            continue;
        }
        // Its doing at every run of its behaviour: a guard's post, one that stands its own
        // (`0x4EFB03`, `0x4F46C9`).
        if mind.active && mind.is_guard() {
            mind.activity = activity::GUARDS;
        } else if mind.stands_by_flags() {
            mind.activity = activity::STANDS;
        }
        if let Some(walk) = walk {
            // A guard decides as it walks too, the others only standing (`0x4EE030`, #608): on a
            // stroll it turns back to its post from farther than 90.
            let walking = (false, Some(spot(walk.target())), in_the_open);
            if mind.active
                && mind.is_guard()
                && !walk.comes_on()
                && let Some((target, why)) = mind.behaves(spot(here), None, walking, &mut minds.rng)
            {
                let target = point(target, here.y);
                debug!(
                    "ai: {entity} {why:?} on its way from {:.0},{:.0} to {:.0},{:.0}",
                    here.x, here.z, target.x, target.z
                );
                walks.push((entity, Walk::to(target, way.level, mind.speed)));
                // A hostile guard's walk takes it back to the AI's state 0 (#3073).
                if mind.on_guard() {
                    mind.active = false;
                }
            }
            continue;
        }
        // Through its door, in or out, it closes it behind it (the sub-states 2 and 3).
        if let Some(door) = mind.door.take() {
            swings.push((door, false));
        }
        // At its door's outer point it opens the door and walks through to its inner point onto
        // the building's floor (the sub-states 9 and 7); one that stopped farther than 350 from
        // the door gives up, to decide again.
        if let Some((door, at, inner)) = mind.entering.take()
            && (at - here).xz().length() <= OPENS_WITHIN
        {
            debug!(
                "ai: {entity} {:?} from {:.0},{:.0} to {:.0},{:.0}",
                Why::GoesIn,
                here.x,
                here.z,
                inner.x,
                inner.z
            );
            mind.door = Some(door);
            swings.push((door, true));
            walks.push((entity, Walk::to(inner, way.level, mind.speed)));
            continue;
        }
        // At the trader it walked to, it talks once (the sub-states 12 and 13), and stays, where
        // the original's goes into a house (docs/decisions/ai.md, "Townsfolk's traders, doors and
        // danger").
        if mind.visiting.take().is_some() {
            commands.entity(entity).insert(Talks);
            continue;
        }
        // A creature with a partner follows it, before its own behaviour, while the hero is
        // within 1,500 of it.
        if let Some(partner) = mind.partner {
            if (here - hero).xz().length() > BEHAVES_WITHIN {
                mind.active = false;
                continue;
            }
            // A child follows the hero only while it stands on a settlement's ground.
            let after_hero = partner == the_hero;
            let found = if after_hero {
                in_settlement.then_some((hero, false, false))
            } else {
                followed.get(&partner).copied()
            };
            let Some((at, partner_walks, partner_trades)) = found else {
                mind.partner = None;
                continue;
            };
            mind.activity = activity::FOLLOWS;
            let why = follow(spot(here), spot(at), after_hero);
            debug!(
                "ai: {entity} {why:?} from {:.0},{:.0} about {partner} at {:.0},{:.0}",
                here.x, here.z, at.x, at.z
            );
            match why {
                Why::Follows if mind.runs_after => {
                    walks.push((entity, Walk::run(at, way.level, mind.run)));
                }
                Why::Follows => walks.push((entity, Walk::to(at, way.level, mind.speed))),
                Why::StepsOff => {
                    let free = |to: Spot| {
                        let cell = paths::cell_of(to.x, to.y);
                        heights.walkable_on(cell, way.level)
                            && !crowd.stops(cell, way.level, entity, false)
                    };
                    match steps_off(spot(at), &mut minds.rng, free) {
                        Some(target) => walks
                            .push((entity, Walk::to(point(target, at.y), way.level, mind.speed))),
                        None => mind.partner = None,
                    }
                }
                Why::Meets => {
                    // They meet: an adult, in one case of two, turns to its partner, which stops
                    // and turns back, and talks.
                    mind.last = Some(partner);
                    mind.partner = None;
                    mind.activity = activity::NONE;
                    if mind.talks_at_meeting(&mut minds.rng) {
                        mind.activity = mind.talks_of(&mut minds.rng);
                        turns.push((entity, at));
                        if !partner_trades {
                            turns.push((partner, here));
                            doings.push((partner, activity::LISTENS));
                            if partner_walks {
                                stops.push(partner);
                            }
                        }
                        // A woman's slap in place of her talk, her attack's clip once (#2251).
                        let distance = (at - here).xz().length();
                        match clips.get(entity) {
                            Ok(npc) if mind.slaps(distance, &mut minds.rng) => {
                                debug!("ai: {entity} slaps {partner} from {distance:.0}");
                                mind.activity = activity::SLAPS;
                                doings.push((partner, activity::SLAPPED));
                                let slot = npc.fidget_slot(minds::Fidget::Attack);
                                commands
                                    .entity(entity)
                                    .insert(crate::npcs::ScriptClip::fidget(slot));
                            }
                            _ => {
                                commands.entity(entity).insert(Talks);
                            }
                        }
                    }
                }
                _ => {
                    mind.last = Some(partner);
                    mind.partner = None;
                }
            }
            continue;
        }
        let crowded = crowd.crowded(paths::cell_of(here.x, here.z), way.level, entity);
        let folk = mind.is_townsperson();
        if mind.active && (here - hero).xz().length() > mind.range(zoom) {
            mind.active = false;
            // At dusk and by night one on the ground goes dormant in a house: by the nearest door
            // within 1,500 of it, put there at once (`0x4F0ADC`, `0x4EEFA0`).
            if folk && evening && way.level.is_none() {
                let door = doors
                    .iter()
                    .map(|&(_, at, _)| at)
                    .filter(|&at| (at - here).xz().length() <= DOOR_WITHIN)
                    .min_by(|a, b| {
                        (*a - here)
                            .length_squared()
                            .total_cmp(&(*b - here).length_squared())
                    });
                if let Some((at, level)) =
                    door.and_then(|door| indoors(door, &heights, &mut minds.rng))
                {
                    debug!(
                        "ai: {entity} {:?} from {:.0},{:.0} to {:.0},{:.0}",
                        Why::PutIn,
                        here.x,
                        here.z,
                        at.x,
                        at.z
                    );
                    commands.entity(entity).insert(PutAt { at, level });
                }
            }
        }
        // A townsperson awake in danger runs from its threat, at every tick it stands.
        if mind.active
            && folk
            && !mind.dialogue
            && let Some(target) = danger(here, &hostiles, &mut minds.rng)
        {
            // Away from a danger (`0x4F1875`).
            mind.activity = activity::WALKS + 1;
            debug!(
                "ai: {entity} {:?} from {:.0},{:.0} to {:.0},{:.0}",
                Why::Flees,
                here.x,
                here.z,
                target.x,
                target.z
            );
            walks.push((entity, Walk::run(target, way.level, mind.run)));
            continue;
        }
        // A townsperson awake decides at a look of the hero's, unless its dialogue holds it.
        if mind.active && folk && nearest.looked && !mind.dialogue {
            let (x, y) = sector(here);
            // The nearest door within 1,500 of the 3 × 3 sectors (`0x4EE300`).
            let nearest_door = doors
                .iter()
                .copied()
                .filter(|&(_, at, _)| {
                    let (ax, ay) = sector(at);
                    (ax - x).abs() <= 1.0
                        && (ay - y).abs() <= 1.0
                        && (at - here).xz().length() <= DOOR_WITHIN
                })
                .min_by(|a, b| {
                    (a.1 - here)
                        .length_squared()
                        .total_cmp(&(b.1 - here).length_squared())
                });
            let door = nearest_door.map(|(_, at, _)| at);
            // Going through it, it opens its door (`0x4FC8D0`).
            let through = |mind: &mut Mind, swings: &mut Vec<(Entity, bool)>| {
                if let Some((door, ..)) = nearest_door {
                    mind.door = Some(door);
                    swings.push((door, true));
                }
            };
            // Going in, it walks to the door's outer point from afar, and within 350 opens it and
            // walks through to its inner point (#431); without the points, through the
            // building's entrance to a cell inside.
            let goes_in = |mind: &mut Mind,
                           swings: &mut Vec<(Entity, bool)>,
                           rng: &mut Rng|
             -> Option<Vec3> {
                match nearest_door.and_then(|door| entering(door, here, &heights)) {
                    Some(Entering::ToDoor(outer, then)) => {
                        mind.entering = Some(then);
                        Some(outer)
                    }
                    Some(Entering::Through(door, inner)) => {
                        mind.door = Some(door);
                        swings.push((door, true));
                        Some(inner)
                    }
                    None => {
                        let (inside, _) = door.and_then(|door| indoors(door, &heights, rng))?;
                        through(mind, swings);
                        Some(inside)
                    }
                }
            };
            // At dusk and by night, or with the Daemon near, on a free cell, it takes the
            // evening's way (`0x4F0F5E`): indoors it walks among the building's cells; outdoors
            // it goes into the house of the nearest door, by stages from afar, and without a
            // door it wanders.
            let before_daemon = daemon_near(daemon.0, spot(here), spot(hero));
            // Before the Daemon, in one case of three, it shows its fear over itself
            // (`0x4F0DAC`, red while the hero has solved fewer than 25 quests).
            if before_daemon
                && !crowded
                && let Some(line) = daemon_fear(&mut minds.rng)
            {
                commands.entity(entity).insert(Says::new(line, FEAR_RED));
            }
            if (evening || before_daemon) && !crowded {
                let inside = way.level.is_some();
                // Outdoors by night it takes a torch; indoors it puts away what it holds in its
                // left hand (`0x4F0FA8`, `0x4F1898`).
                if let Some(torch) = mind.townsperson_torch(!inside, (!inside, night), left()) {
                    torches.push((entity, torch));
                }
                // Its doing: the evening's way taken, then its stage's, one more before the
                // Daemon (`0x4F0F72`, `0x4F1037`, `0x4F1AAB`, `0x4F2FEB`).
                let danger = u16::from(before_daemon);
                mind.activity = activity::SLEEPS + danger;
                let stage = match evening_way(spot(here), inside, door.map(spot), &mut minds.rng) {
                    Evening::Indoors => {
                        mind.activity = activity::NERVOUS;
                        indoors(here, &heights, &mut minds.rng).map(|(to, _)| (to, Why::Wanders))
                    }
                    Evening::Toward(to) => {
                        mind.activity = activity::TO_DOOR + danger;
                        Some((point(to, here.y), Why::GoesIn))
                    }
                    Evening::GoesIn => {
                        mind.activity = activity::TO_DOOR + danger;
                        goes_in(&mut mind, &mut swings, &mut minds.rng).map(|to| (to, Why::GoesIn))
                    }
                    Evening::Wanders(to) => {
                        mind.activity = activity::WALKS + danger;
                        Some((point(to, here.y), Why::Wanders))
                    }
                };
                if let Some((target, why)) = stage {
                    debug!(
                        "ai: {entity} {why:?} from {:.0},{:.0} to {:.0},{:.0}",
                        here.x, here.z, target.x, target.z
                    );
                    walks.push((entity, Walk::to(target, way.level, mind.speed)));
                }
                continue;
            }
            // On the day's way it puts away what its left hand holds, but outdoors by night
            // (`0x4F1ACF`).
            if let Some(torch) = mind.townsperson_torch(false, (way.level.is_none(), night), left())
            {
                torches.push((entity, torch));
            }
            // Indoors on a free cell, it leaves by day (`0x4F1AB9`).
            if way.level.is_some()
                && !crowded
                && let Some(target) = door.and_then(|door| outdoors(door, &heights, &mut minds.rng))
            {
                debug!(
                    "ai: {entity} {:?} from {:.0},{:.0} to {:.0},{:.0}",
                    Why::GoesOut,
                    here.x,
                    here.z,
                    target.x,
                    target.z
                );
                through(&mut mind, &mut swings);
                mind.activity = activity::GOES_OUT;
                walks.push((entity, Walk::to(target, way.level, mind.speed)));
                continue;
            }
            let nearest = |of: &[(Entity, Vec3)]| {
                of.iter()
                    .filter(|&&(other, at)| {
                        let (ax, ay) = sector(at);
                        other != entity
                            && Some(other) != mind.last
                            && (ax - x).abs() <= 1.0
                            && (ay - y).abs() <= 1.0
                    })
                    .min_by(|a, b| {
                        let da = (a.1 - here).length_squared();
                        da.total_cmp(&(b.1 - here).length_squared())
                    })
                    .copied()
            };
            let (human, animal) = (nearest(&humans), nearest(&animals));
            let near_traders = traders
                .iter()
                .filter(|&&(other, at)| {
                    let (ax, ay) = sector(at);
                    other != entity && (ax - x).abs() <= 1.0 && (ay - y).abs() <= 1.0
                })
                .map(|&trader| (trader, spot(trader.1)))
                .collect();
            let trader = trader_for(spot(here), near_traders);
            match townsperson(&mind, human, animal, trader, &mut minds.rng) {
                Decision::Visits((animal, target)) => {
                    mind.last = Some(animal);
                    mind.activity = activity::ANIMAL;
                    debug!(
                        "ai: {entity} {:?} from {:.0},{:.0} to {:.0},{:.0}",
                        Why::Visits,
                        here.x,
                        here.z,
                        target.x,
                        target.z
                    );
                    walks.push((entity, Walk::to(target, way.level, mind.speed)));
                    continue;
                }
                Decision::GoesHome => {
                    mind.activity = activity::GOES_HOME;
                    let target = point(mind.home, here.y);
                    debug!(
                        "ai: {entity} {:?} from {:.0},{:.0} to {:.0},{:.0}",
                        Why::GoesHome,
                        here.x,
                        here.z,
                        target.x,
                        target.z
                    );
                    walks.push((entity, Walk::to(target, way.level, mind.speed)));
                    continue;
                }
                Decision::Partner((partner, _), home) => {
                    debug!("ai: {entity} {:?} {partner}", Why::TakesPartner);
                    mind.partner = Some(partner);
                    mind.runs_after = false;
                    mind.activity = if home {
                        activity::GOES_HOME
                    } else {
                        activity::ROMANCE
                    };
                    if home {
                        let home = point(mind.home, here.y);
                        walks.push((entity, Walk::to(home, way.level, mind.speed)));
                    }
                    continue;
                }
                Decision::Trade((trader, at)) => {
                    debug!(
                        "ai: {entity} {:?} from {:.0},{:.0} at {trader} at {:.0},{:.0}",
                        Why::Trades,
                        here.x,
                        here.z,
                        at.x,
                        at.z
                    );
                    mind.last = Some(trader);
                    mind.visiting = Some(trader);
                    mind.activity = activity::TRADER;
                    let walk = Walk::to(at, way.level, mind.speed).within(AT_THE_TRADER);
                    walks.push((entity, walk));
                    continue;
                }
                Decision::Child => {
                    // A child flees before a hero of the type 7, the Vampiress by night, as
                    // before the Daemon's 9 (`0x4F2AD7`, #2897).
                    let feared =
                        daemon.0 || forms.as_deref().is_some_and(crate::forms::Forms::by_night);
                    let does = child(
                        &mind,
                        spot(here),
                        (spot(hero), feared, shoos),
                        in_settlement,
                        &mut minds.rng,
                    );
                    debug!("ai: {entity} {:?} {does:?}", Why::Plays);
                    match does {
                        Child::Plays(to) => {
                            mind.activity = activity::WALKS;
                            let to = point(to, here.y);
                            walks.push((entity, Walk::run(to, way.level, mind.run)));
                            playing.push((entity, here));
                        }
                        Child::FollowsHero => {
                            mind.partner = Some(the_hero);
                            mind.runs_after = true;
                        }
                        Child::LooksUp => turns.push((entity, hero)),
                        Child::Speaks(line) => {
                            turns.push((entity, hero));
                            commands.entity(entity).insert(Says::new(line, CHILD_GREEN));
                        }
                        Child::RunsUp(to) | Child::Flees(to) => {
                            let to = point(to, here.y);
                            walks.push((entity, Walk::run(to, way.level, mind.run)));
                        }
                        Child::Shooed(to) => {
                            mind.activity = activity::WALKS;
                            shooed = Some(here);
                            let to = point(to, here.y);
                            walks.push((entity, Walk::run(to, way.level, mind.run)));
                        }
                        Child::Stays => {}
                    }
                    continue;
                }
                Decision::House => {
                    mind.activity = activity::GOES_IN;
                    if let Some(target) = goes_in(&mut mind, &mut swings, &mut minds.rng) {
                        debug!(
                            "ai: {entity} {:?} from {:.0},{:.0} to {:.0},{:.0}",
                            Why::GoesIn,
                            here.x,
                            here.z,
                            target.x,
                            target.z
                        );
                        walks.push((entity, Walk::to(target, way.level, mind.speed)));
                        continue;
                    }
                }
                Decision::Juggle => {
                    let (instrument, on) = mind.juggles(&mut minds.rng);
                    debug!("ai: {entity} {:?} {instrument} on {on}", Why::Juggles);
                    commands.entity(entity).insert(Plays { instrument, on });
                    continue;
                }
                Decision::Stop => {
                    mind.activity = activity::NONE;
                    // Standing by a door's point, it walks off within 250 of itself as a trader's
                    // visit's sub-state 12, to talk once where it stands again (`0x4F332F`,
                    // docs/re/ai.md, "The roll", #3424).
                    if by_door_point(nearest_door, here) {
                        mind.activity = activity::WALKS_OFF;
                        mind.visiting = Some(entity);
                        let target = point(minds::walks_off(spot(here), &mut minds.rng), here.y);
                        debug!(
                            "ai: {entity} walks off a door's point from {:.0},{:.0} to {:.0},{:.0}",
                            here.x, here.z, target.x, target.z
                        );
                        walks.push((entity, Walk::to(target, way.level, mind.speed)));
                        continue;
                    }
                }
            }
        }
        if !mind.active || folk {
            let on_ground = way.level.is_none();
            if let Some(target) = mind.parts(spot(here), (crowded, on_ground), &mut minds.rng) {
                let target = point(target, here.y);
                debug!(
                    "ai: {entity} {:?} from {:.0},{:.0} to {:.0},{:.0}",
                    Why::Parts,
                    here.x,
                    here.z,
                    target.x,
                    target.z
                );
                // The standing action's (`0x544890`), with the AI's state 0; a flyer's flown
                // (`flight::take_wing`).
                walks.push((entity, Walk::to(target, way.level, mind.speed).idle()));
            }
            continue;
        }
        // Held, a cage's or a knockback's, it moves not: the hold's factor of the clips (`+0x2C9`,
        // the effect 15's value, 0 for the plant cage) times its walk's speed (**hypothesis**,
        // `0x4F510F`; docs/re/ai.md, "Open"), which a decision of its behaviour leaves standing.
        if mind.held() {
            continue;
        }
        // The nearest it fears in its own sector, each within its distance; it flees whatever
        // the hero wears, the effect 855's and set 41's terror being a swooping flyer's (#3596).
        let fear = mind.nearest_feared(
            spot(here),
            feared
                .iter()
                .filter(|&&(other, at, ..)| other != entity && sector(at) == sector(here))
                .map(|&(_, at, animal, flags, attacks)| minds::Fearsome {
                    at: spot(at),
                    animal,
                    flags,
                    attacks,
                }),
        );
        let Some((target, why)) = mind.behaves(
            spot(here),
            fear,
            (crowded, None, in_the_open),
            &mut minds.rng,
        ) else {
            continue;
        };
        let target = point(target, here.y);
        debug!(
            "ai: {entity} {why:?} from {:.0},{:.0} to {:.0},{:.0}",
            here.x, here.z, target.x, target.z
        );
        walks.push((
            entity,
            match mind.gait(why, in_the_open) {
                minds::Gait::Flies => Walk::fly(target, way.level, mind.fly),
                minds::Gait::Runs => Walk::run(target, way.level, mind.run),
                minds::Gait::Walks => Walk::to(target, way.level, mind.speed),
            },
        ));
        // A hostile guard's walk back to its post or its stroll takes it back to the AI's state 0
        // (#3073).
        if mind.on_guard() {
            mind.active = false;
        }
    }
    let sent: std::collections::HashSet<Entity> = walks.iter().map(|&(entity, _)| entity).collect();
    for (entity, walk) in walks {
        commands.entity(entity).insert(walk);
    }
    for entity in stops {
        commands.entity(entity).remove::<Walk>();
    }
    for (door, open) in swings {
        commands.entity(door).insert(Swing::to(open));
    }
    // Its torch taken or put away as it rearms (`npcs::Rearms`), unless it rearms already.
    for (entity, torch) in torches {
        debug!("ai: {entity} {torch:?} a torch");
        commands.entity(entity).insert_if_new(Rearms::torch(torch));
    }
    // The other idle children within 600 of one at play, without a dialogue, run after it
    // (`0x4EE820`); not one sent off this tick (`sent`).
    for (leader, at) in playing {
        let (lx, ly) = sector(at);
        for (entity, transform, _, mut mind, walk) in &mut creatures {
            let (x, y) = sector(transform.translation);
            if entity != leader
                && walk.is_none()
                && !sent.contains(&entity)
                && mind.is_child()
                && mind.partner.is_none()
                && !mind.dialogue
                && (x - lx).abs() <= 1.0
                && (y - ly).abs() <= 1.0
                && (transform.translation - at).xz().length() <= PLAYMATES
            {
                debug!("ai: {entity} {:?} {leader}", Why::Gathers);
                mind.partner = Some(leader);
                mind.runs_after = true;
            }
        }
    }
    for (entity, doing) in doings {
        if let Ok((_, _, _, mut mind, ..)) = creatures.get_mut(entity) {
            mind.activity = doing;
        }
    }
    for (entity, at) in turns {
        if let Ok((_, mut transform, ..)) = creatures.get_mut(entity) {
            transform.rotation = crate::hero::facing(at - transform.translation);
        }
    }
    if let Some(toward) = shooed {
        commands.entity(the_hero).insert(Shoos {
            toward,
            left: SHOO_HOLDS,
        });
    }
}

/// How long the hero's stance's bit of a shoo holds, until its next look clears it (**estimate**:
/// its look every 0.3 s; seen 0.08 to 0.33 s, #3517).
const SHOO_HOLDS: f32 = 0.3;

/// The hero's stance's bit of a shoo (`0x4F2C8C`; docs/re/ai.md, "Children and jugglers",
/// #3150): it faces the child that it shooed last, `toward`, at once (`0x44DD90`), and its ring
/// shows while the bit holds (`health_rings`), `left` seconds more.
#[derive(Component, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Shoos {
    pub toward: Vec3,
    pub left: f32,
}

/// Turns the hero to the child it shoos at once, no clip turning it, and lets the shoo's bit go
/// as its time runs out.
fn face_shooed(
    mut commands: Commands,
    time: Res<Time>,
    mut heroes: Query<(Entity, &mut Transform, &mut Shoos), With<Body>>,
) {
    for (hero, mut transform, mut shoos) in &mut heroes {
        if shoos.is_changed() {
            let to = (shoos.toward - transform.translation).with_y(0.0);
            if to.length_squared() > 0.0 {
                transform.rotation = crate::hero::facing(to);
            }
        }
        shoos.left -= time.delta_secs();
        if shoos.left <= 0.0 {
            commands.entity(hero).remove::<Shoos>();
        }
    }
}

/// Raises the group's alarm of each creature in a group that a strike reaches, as the strike's
/// event comes (`0x5337CE`, `0x46AE70`), whatever the strike resolves to: the group's other idle
/// members (`minds::Mind::idle`) take its foe for theirs where it fights a creature, come at the
/// hero where it fights the hero, and else wake, to decide at their next tick (#669); so the
/// start's bears by the boss 689, hostile guards without the flags' bit 0 that the hero's look
/// does not wake, wake as the boss strikes one of them and take their guards' behaviour. A
/// member in mid-shot that the alarm of a member that fights reaches takes it in its shots' chain
/// and aims its playing shot at that member's foe (docs/re/fight.md, "The shots' chain", #3313).
fn alarm(
    mut commands: Commands,
    mut hits: MessageReader<crate::fight::Hit>,
    mut creatures: Query<(Entity, &mut Mind)>,
    ridden: Query<&crate::riding::Ridden>,
    (mut shots, heroes): (Query<&mut Strikes>, Query<Entity, With<Body>>),
) {
    let the_hero = heroes.iter().next();
    // The struck members, their groups and whom they fight: a creature, the hero, or no one.
    let mut alarms: Vec<(Entity, u32, Option<Option<Entity>>)> = Vec::new();
    for hit in hits.read() {
        if let Ok((_, mind)) = creatures.get(hit.target)
            && let Some(group) = mind.group
            && !alarms.iter().any(|&(struck, ..)| struck == hit.target)
        {
            let fights = mind.fighting().then(|| mind.foe());
            alarms.push((hit.target, group, fights));
        }
    }
    for (struck, group, fights) in alarms {
        let mut woken = 0;
        // The riders that their horses send the event `0x100` on to, and whom at.
        let mut forwarded: Vec<(Entity, Option<Entity>)> = Vec::new();
        for (entity, mut mind) in &mut creatures {
            if entity == struck {
                continue;
            }
            if let (Some(foe), Ok(mut strikes)) = (fights, shots.get_mut(entity))
                && strikes.is_shot()
            {
                if let Some(target) = foe.or(the_hero).filter(|&target| target != entity)
                    && mind.called_mid_shot(group)
                {
                    debug!(
                        "ai: {entity} is called in mid-shot, its chain {}",
                        mind.chain()
                    );
                    strikes.retarget(target);
                }
                continue;
            }
            // A ridden horse in the world sends the event `0x100` on to its rider, a creature
            // that is no horse (`0x52FA63`), which takes it as any creature does; the hero's horse
            // and the hero hold `+0x200` bit `0x10` while it rides, and ignore it (#2401).
            if let (Some(foe), Ok(ridden)) = (fights, ridden.get(entity))
                && mind.group == Some(group)
                && mind.idle()
            {
                if ridden.rider != struck && foe != Some(ridden.rider) {
                    forwarded.push((ridden.rider, foe));
                }
                continue;
            }
            let heard = match fights {
                Some(Some(foe)) if foe != entity => {
                    let joins = mind.joins(group);
                    if joins {
                        mind.foe = Some(foe);
                    }
                    joins
                }
                Some(Some(_)) => false,
                // Only a member that hears it comes at the hero; any other, a follower fighting a
                // creature among them, keeps its foe (#1638).
                Some(None) => {
                    let hears = mind.hears_alarm(group);
                    if hears {
                        mind.foe = None;
                    }
                    hears
                }
                // Woken, its sight runs where it sees the hero (`0x46B336`, #3149).
                None => {
                    let wakes = mind.wakes(group);
                    if wakes {
                        commands.entity(entity).insert(Woken);
                    }
                    wakes
                }
            };
            woken += usize::from(heard);
        }
        for (rider, foe) in forwarded {
            if let Ok((_, mut mind)) = creatures.get_mut(rider) {
                let goes = match foe {
                    Some(foe) => mind.send_at(foe),
                    None => mind.send_at_hero(),
                };
                if goes {
                    debug!(
                        "ai: {rider}'s horse sends it its group {group}'s alarm against {foe:?}"
                    );
                }
                woken += usize::from(goes);
            }
        }
        if woken > 0 {
            debug!("ai: {struck} is struck and raises its group {group}'s alarm, {woken} answer");
        }
    }
}

/// The nearest creature to `here` of the 3 × 3 sectors about it, of `others`, that the creature
/// `entity` of `mind` is hostile to (`0x4EEE40`, a guard's foe), straight from it.
fn nearest_foe(entity: Entity, here: Vec3, mind: &Mind, others: &[Other]) -> Option<Other> {
    let (x, y) = sector_of(here);
    others
        .iter()
        .filter(|other| {
            let (ox, oy) = sector_of(other.at);
            other.entity != entity
                && (ox - x).abs() <= 1.0
                && (oy - y).abs() <= 1.0
                && other.hated_by(mind)
        })
        .min_by(|a, b| {
            let (da, db) = ((a.at - here).xz().length(), (b.at - here).xz().length());
            da.total_cmp(&db)
        })
        .copied()
}

/// The creatures' thoughts, behaviours and fights at a tick of the world, after which the
/// followers keep by their masters (`followers`).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Thinks;

/// The creatures' life and their way of fighting.
pub struct AiPlugin;

/// Turns each creature of a dialogue that stands to the hero at each tick, wherever the hero is
/// (the standing action `0x544890` at `0x5464C4`–`0x546565`, #2543): one of a dialogue below
/// 100,000 (`Talk`; the stables' and the trader's stamps are none), no horse (`0x4266F0`, the
/// class 4), in the world, turned at once along the line to the hero (`0x44DD90`). One that
/// walks, strikes, plays a talk of its own or dies stands no more.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn face(
    heroes: Query<&Transform, With<Body>>,
    mut talkers: Query<
        (&mut Transform, &Mind),
        (
            With<crate::npcs::Talk>,
            Without<Body>,
            Without<Walk>,
            Without<Strikes>,
            Without<Talks>,
            Without<crate::npcs::Dies>,
        ),
    >,
) {
    let Ok(hero) = heroes.single() else {
        return;
    };
    for (mut transform, mind) in &mut talkers {
        if mind.class == opensacred_rules::horses::CLASS {
            continue;
        }
        let to = hero.translation - transform.translation;
        if to.xz().length_squared() > f32::EPSILON {
            transform.rotation = crate::hero::facing(to);
        }
    }
}

/// A creature that a wake other than the hero's look woke this frame (`sdk_WAKE`'s other callers;
/// docs/re/ai.md, "Dormant and awake", #3149): `CreateNPC`'s of the flags' bit 0 as it is made,
/// `SetNPCState`'s writing that bit, the group's alarm at an idle member, the hiring and the
/// undead's rising; its sight runs where it sees the hero ([`sight`]), and the mark goes.
#[derive(
    Component, Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize,
)]
pub struct Woken;

impl bevy::ecs::entity::MapEntities for Woken {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, _: &mut E) {}
}

impl crate::doings::Saves for Woken {
    type Saved = Woken;

    fn saved(&self, _: &crate::doings::Moment) -> Option<Self> {
        Some(*self)
    }

    fn again(saved: Self, _: &crate::doings::Moment) -> Option<Self> {
        Some(saved)
    }
}

impl bevy::ecs::entity::MapEntities for Shoos {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, _: &mut E) {}
}

impl crate::doings::Saves for Shoos {
    type Saved = Shoos;

    fn saved(&self, _: &crate::doings::Moment) -> Option<Self> {
        Some(*self)
    }

    fn again(saved: Self, _: &crate::doings::Moment) -> Option<Self> {
        Some(saved)
    }
}

/// Runs each creature's sight function as it first sees the hero (`sdk_WAKE` `0x59F580`, #2475;
/// docs/re/ai.md, "Dormant and awake"): at each of the hero's looks, a creature of a sight
/// pending (`minds::SIGHT`) that the look wakes (`minds::look_wakes`), and at each of the other
/// wakes a creature [`Woken`] that fights no one (#3149), that sees the hero within 600 along x
/// and 400 along y and within 1 rad of its heading on the screen (`minds::sees`), by a straight
/// way (`0x4FCC80`), runs its first `See:` function, and its bit clears, its kept placement's
/// with it (`0x59F78F`), whether it had one or not. Quest 11 of the Underworld is entered so, as
/// its guards see the hero come through the gate.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn sight(
    mut commands: Commands,
    nearest: Res<Nearest>,
    heroes: Query<(&Transform, &Body)>,
    mut creatures: Query<
        (
            Entity,
            &Transform,
            &mut Mind,
            &InTheWay,
            (Option<&crate::npcs::OnDeath>, Option<&crate::npcs::Placed>),
            Has<Woken>,
        ),
        Without<crate::npcs::Dies>,
    >,
    heights: Res<Heights>,
    mut npcs: ResMut<Npcs>,
) {
    let Ok((hero, body)) = heroes.single() else {
        return;
    };
    let at = spot(hero.translation);
    let moves = body.target().is_some();
    for (entity, transform, mut mind, way, (on_death, placed), woken) in &mut creatures {
        if woken {
            commands.entity(entity).remove::<Woken>();
        }
        if mind.flags & minds::SIGHT == 0 {
            continue;
        }
        let here = spot(transform.translation);
        let (hostile, fighting) = (mind.hostile(), mind.fighting());
        // The look's wake, or another's of one that fights no one (`0x59F580`).
        let looks = nearest.looked
            && minds::look_wakes(hostile, mind.flags, fighting, (here - at).length(), moves);
        if !looks && !(woken && !fighting) {
            continue;
        }
        let heading = transform.rotation * Vec3::X;
        if !minds::sees(here, Spot::new(heading.x, heading.z), at)
            // A line of sight, over the water and the fences (`0x59F6B2`, #3358), on its floor of
            // the building at each cell (#3852).
            || !paths::straight(transform.translation.xz(), hero.translation.xz(), |cell| {
                let floor = heights.floor_number(way.level);
                line_passes(&heights, floor, cell, |cell, at| heights.flown_onto(cell, at))
            })
        {
            continue;
        }
        mind.flags &= !minds::SIGHT;
        if let Some(index) = placed.and_then(crate::npcs::Placed::kept) {
            npcs.sighted(index);
        }
        let Some(function) = on_death
            .and_then(|on_death| on_death.0.sight())
            .map(str::to_owned)
        else {
            continue;
        };
        info!(
            "ai: {entity}, a {}, sees the hero and runs {function}",
            mind.creature
        );
        commands.queue(move |world: &mut World| {
            if let Err(e) = crate::scripts::run_function(world, &function) {
                warn!("ai: {e}");
            }
        });
    }
}

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Minds>()
            .init_resource::<Daemon>()
            .init_resource::<FightRules>()
            .add_systems(
                Update,
                (open, sight, think, aside, behave, pursue, face)
                    .chain()
                    .in_set(Thinks)
                    .after(crate::crowd::gather)
                    .after(crate::crowd::look)
                    .in_set(Step::Crowd)
                    .run_if(crate::frame::ticked),
            )
            .add_systems(
                Update,
                (alarm, turn_on_attackers, run_off)
                    .in_set(Step::Crowd)
                    .before(think),
            )
            .add_systems(Update, swoop_falls.in_set(Step::Crowd).before(think))
            .add_message::<SwoopPassed>()
            .add_systems(
                Update,
                pass_terror
                    .after(pursue)
                    .in_set(Step::Crowd)
                    .run_if(crate::frame::ticked),
            )
            .add_systems(Update, face_shooed.in_set(Step::Crowd).after(behave));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cast_at_an_enemy_the_ground_or_the_dead_wants_its_caster_s_level() {
        // On an enemy, at a point, on the dead: tested; on a friend, on itself, of none: not.
        assert!([1, 3, 4].into_iter().all(levels_tested));
        assert!(![0, 2, 5].into_iter().any(levels_tested));
    }

    #[test]
    fn a_swooping_bat_struck_aloft_may_fall_and_a_blow_of_nothing_never_fells_it() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<crate::fight::Struck>()
            .init_resource::<Minds>()
            .add_systems(Update, swoop_falls);
        let bat = |app: &mut App, swooping: bool| {
            let mut mind = Mind::new(
                Vec3::ZERO,
                0x1,
                (false, false),
                None,
                [70.0, 180.0],
                (595, 0),
            );
            mind.thinks.distracts();
            assert!(mind.thinks.sent_at());
            mind.thinks.flying = swooping;
            let health = crate::fight::Health {
                current: 0,
                most: 100,
                level: 10,
            };
            app.world_mut().spawn((mind, health)).id()
        };
        let swooping = bat(&mut app, true);
        let landed = bat(&mut app, false);
        let missed = bat(&mut app, true);
        for (target, damage) in [(swooping, 1_000), (landed, 1_000), (missed, 0)] {
            app.world_mut().write_message(crate::fight::Struck {
                target,
                damage,
                most: 100,
                shooter: None,
            });
        }
        app.update();
        let clip = |entity| app.world().get::<crate::npcs::ScriptClip>(entity).is_some();
        // Its whole health in one blow: a chance of 100 in 101.
        assert!(clip(swooping));
        assert!(!clip(landed) && !clip(missed));
    }

    #[test]
    fn a_struck_guard_runs_off_from_the_living_foe_it_fights_and_the_others_stand() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<crate::fight::Hit>()
            .init_resource::<Minds>()
            .add_systems(Update, run_off);
        let health = |current| crate::fight::Health {
            current,
            most: 100,
            level: 1,
        };
        let world = app.world_mut();
        let goblin = world
            .spawn((Transform::from_xyz(100.0, 0.0, 0.0), health(50)))
            .id();
        let dead = world
            .spawn((Transform::from_xyz(100.0, 0.0, 0.0), health(0)))
            .id();
        // A guard of the side 7, a friend of the heroes, fighting `foe` where it is given.
        let guard = |world: &mut World, foe: Option<Entity>, horse: bool| {
            let mut mind = Mind::new(
                Vec3::ZERO,
                0x4000,
                (false, true),
                None,
                [70.0, 180.0],
                (689, 14),
            );
            mind.side = 7;
            if let Some(foe) = foe {
                assert!(mind.send_at(foe));
            }
            let mut guard = world.spawn((mind, Transform::default(), InTheWay::of(0, 0, None)));
            if horse {
                guard.insert(crate::riding::Horse { class: 1 });
            }
            guard.id()
        };
        let fights = guard(world, Some(goblin), false);
        let fights_the_dead = guard(world, Some(dead), false);
        let calm = guard(world, None, false);
        let horse = guard(world, Some(goblin), true);
        for target in [fights, fights_the_dead, calm, horse] {
            assert!(!app.world().get::<Mind>(target).unwrap().hostile());
            app.world_mut().write_message(crate::fight::Hit {
                attacker: goblin,
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
                marks: false,
            });
        }
        app.update();
        // It runs 700 beyond its place away from its target, its fight and its target kept.
        let walk = app.world().get::<Walk>(fights).unwrap();
        assert!(walk.runs());
        assert!((walk.target() - Vec3::new(-700.0, 0.0, 0.0)).length() < 1e-3);
        let mind = app.world().get::<Mind>(fights).unwrap();
        assert!(mind.fighting() && mind.foe() == Some(goblin));
        for stands in [fights_the_dead, calm, horse] {
            assert!(app.world().get::<Walk>(stands).is_none());
        }
    }

    #[test]
    fn the_heros_follower_struck_by_a_goblin_turns_on_it_and_not_on_the_hero() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<crate::fight::Hit>()
            .init_resource::<Minds>()
            .add_systems(Update, turn_on_attackers);
        let hero = app.world_mut().spawn(Body::standing()).id();
        let goblin = app
            .world_mut()
            .spawn(Mind::new(
                Vec3::ZERO,
                0x4000,
                (false, false),
                None,
                [70.0, 180.0],
                (76, 2),
            ))
            .id();
        // A goblin hired as the hiring makes it: following, of the hired side.
        let follower = |app: &mut App, master: Entity| {
            let mut mind = Mind::new(Vec3::ZERO, 0, (false, false), None, [70.0, 180.0], (76, 2));
            mind.flags = minds::FOLLOWS;
            mind.side = crate::followers::HIRED_SIDE;
            app.world_mut()
                .spawn((mind, crate::followers::Follower::of(master)))
                .id()
        };
        let hit = |attacker, target| crate::fight::Hit {
            attacker,
            target,
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
            marks: false,
        };
        let struck = follower(&mut app, hero);
        let by_the_hero = follower(&mut app, hero);
        // Another's follower, the hero's not, is no creature the tail admits.
        let stranger = app.world_mut().spawn_empty().id();
        let others = follower(&mut app, stranger);
        for (attacker, target) in [(goblin, struck), (hero, by_the_hero), (goblin, others)] {
            assert!(!app.world().get::<Mind>(target).unwrap().hostile());
            app.world_mut().write_message(hit(attacker, target));
        }
        app.update();
        let mind = |entity| app.world().get::<Mind>(entity).unwrap();
        assert!(mind(struck).fighting() && mind(struck).foe() == Some(goblin));
        assert!(!mind(by_the_hero).fighting());
        assert!(!mind(others).fighting());
    }

    #[test]
    fn a_dormant_goblin_struck_by_the_hero_turns_on_it() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<crate::fight::Hit>()
            .init_resource::<Minds>()
            .add_systems(Update, turn_on_attackers);
        let hero = app.world_mut().spawn(Body::standing()).id();
        let goblin = app
            .world_mut()
            .spawn(Mind::new(
                Vec3::ZERO,
                0x4000,
                (false, false),
                None,
                [70.0, 180.0],
                (76, 2),
            ))
            .id();
        assert!(app.world().get::<Mind>(goblin).unwrap().hostile());
        app.world_mut().write_message(crate::fight::Hit {
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
            marks: false,
        });
        app.update();
        let mind = app.world().get::<Mind>(goblin).unwrap();
        assert!(mind.fighting() && mind.foe().is_none());
    }

    #[test]
    fn the_hero_goes_unseen_in_the_water_form_or_the_dark_elfs_set_while_it_attacks_nothing() {
        let water = |after| crate::spells::Unseen { after, left: 10.0 };
        // The water form once it starts, attacking nothing, whatever the hero's type (#3084).
        assert!(unseen_hero(Some(3), (Some(&water(0.0)), None), false));
        assert!(!unseen_hero(Some(3), (Some(&water(0.5)), None), false));
        // Attacking, it is seen and stopped at once, the form still on it.
        assert!(!unseen_hero(Some(3), (Some(&water(0.0)), None), true));
        // The Dark Elf without his set's seven pieces, and the others, go seen.
        let holding = crate::fight::Holding::default();
        assert!(!unseen_hero(Some(DARK_ELF), (None, Some(&holding)), false));
        assert!(!unseen_hero(Some(3), (None, Some(&holding)), false));
        assert!(!unseen_hero(None, (None, None), false));
    }

    #[test]
    fn a_creature_of_a_dialogue_that_stands_turns_to_the_hero_but_a_horse() {
        let mut app = App::new();
        app.add_systems(Update, face);
        app.world_mut().spawn((
            Transform::from_xyz(100.0, 0.0, 0.0),
            crate::hero::Body::standing(),
        ));
        let mind = |class| {
            Mind::new(
                Vec3::ZERO,
                0,
                (false, true),
                None,
                [70.0, 180.0],
                (679, class),
            )
        };
        let talk = || crate::npcs::Talk("novice".to_owned());
        let away = Transform::from_xyz(0.0, 0.0, 0.0).with_rotation(Quat::from_rotation_y(2.0));
        let novice = app.world_mut().spawn((away, mind(3), talk())).id();
        let horse = app
            .world_mut()
            .spawn((away, mind(opensacred_rules::horses::CLASS), talk()))
            .id();
        let mute = app.world_mut().spawn((away, mind(3))).id();
        let talking = app.world_mut().spawn((away, mind(3), talk(), Talks)).id();
        app.update();
        let turned = |entity| app.world().get::<Transform>(entity).map(|t| t.rotation);
        assert_eq!(turned(novice), Some(crate::hero::facing(Vec3::X)));
        for unturned in [horse, mute, talking] {
            assert_eq!(turned(unturned), Some(away.rotation));
        }
    }

    #[test]
    fn a_guard_takes_the_nearest_creature_hostile_to_it_of_the_sectors_about_it() {
        // The start's boss, a guard on the side 7, and the bears of the side 14 about it.
        let mut boss = Mind::new(
            Vec3::ZERO,
            0x4000,
            (false, true),
            None,
            [70.0, 180.0],
            (689, 14),
        );
        boss.side = 7;
        let other = |bits, x: f32, side, creature| Other {
            entity: Entity::from_raw_u32(bits).unwrap(),
            at: Vec3::new(x, 0.0, 10.0),
            side,
            creature,
            class: 0,
            level: 11,
            size: 130,
            turned: false,
        };
        let me = Entity::from_raw_u32(1).unwrap();
        let side = crate::iso::CELL * 64.0;
        let others = [
            other(1, 0.0, 7, 689),
            other(2, 400.0, 14, 537),
            other(3, 300.0, 7, 297),
            other(4, 250.0, 6, 520),
            other(5, 350.0, 14, 538),
            other(6, -2.5 * side, 2, 76),
        ];
        // Not itself, a friend of its side, a farm animal, nor one beyond the 3 × 3 sectors.
        let foe = nearest_foe(me, Vec3::new(10.0, 0.0, 10.0), &boss, &others).unwrap();
        assert_eq!(foe.entity, Entity::from_raw_u32(5).unwrap());
        assert_eq!(
            nearest_foe(me, Vec3::ZERO, &boss, &others[..1]).map(|o| o.entity),
            None
        );
    }

    #[test]
    fn a_groups_alarm_leaves_a_follower_its_foe() {
        let mut app = App::new();
        app.add_message::<crate::fight::Hit>()
            .add_systems(Update, alarm);
        let hostile = |group| {
            let mut mind = Mind::new(
                Vec3::ZERO,
                0x1,
                (false, true),
                Some(group),
                [70.0, 180.0],
                (567, 2),
            );
            mind.side = 2;
            mind
        };
        // A wolf of the group 10 that fights the hero, and its idle fellow.
        let mut wolf = hostile(10);
        assert!(wolf.hears_alarm(10));
        let wolf = app.world_mut().spawn(wolf).id();
        let fellow = app.world_mut().spawn(hostile(10)).id();
        // The elf archer that follows the hero, sent at another creature (#1638).
        let foe = app.world_mut().spawn_empty().id();
        let mut archer = Mind::new(
            Vec3::ZERO,
            minds::FOLLOWS,
            (false, true),
            None,
            [70.0, 180.0],
            (134, 12),
        );
        archer.side = 3;
        assert!(archer.send_at(foe));
        let archer = app.world_mut().spawn(archer).id();
        let hero = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(crate::fight::Hit {
            attacker: hero,
            target: wolf,
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
            marks: false,
        });
        app.update();
        // The fellow comes at the hero; the archer, of no group, keeps its foe.
        let fellow = app.world().get::<Mind>(fellow).unwrap();
        assert!(fellow.fighting() && fellow.foe().is_none());
        let archer = app.world().get::<Mind>(archer).unwrap();
        assert!(archer.fighting());
        assert_eq!(archer.foe(), Some(foe));
    }

    #[test]
    fn a_ridden_horse_sends_its_groups_alarm_on_to_its_rider() {
        let mut app = App::new();
        app.add_message::<crate::fight::Hit>()
            .add_systems(Update, alarm);
        let mind = |group, (creature, class)| {
            let mut mind = Mind::new(
                Vec3::ZERO,
                0x1,
                (false, true),
                group,
                [70.0, 180.0],
                (creature, class),
            );
            mind.side = 5;
            mind
        };
        // A black horse of the group 10 and its rider, of none; a skeleton of the group that
        // fights a creature.
        let rider = app.world_mut().spawn(mind(None, (256, 5))).id();
        let horse = app
            .world_mut()
            .spawn((
                mind(Some(10), (261, 4)),
                crate::riding::Ridden::by(rider, None),
            ))
            .id();
        let foe = app.world_mut().spawn_empty().id();
        let mut skeleton = mind(Some(10), (32, 5));
        assert!(skeleton.send_at(foe));
        let skeleton = app.world_mut().spawn(skeleton).id();
        app.world_mut().write_message(crate::fight::Hit {
            attacker: foe,
            target: skeleton,
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
            marks: false,
        });
        app.update();
        // The event goes on to the rider, which fights the skeleton's foe; the horse fights not.
        let rider = app.world().get::<Mind>(rider).unwrap();
        assert!(rider.fighting());
        assert_eq!(rider.foe(), Some(foe));
        assert!(!app.world().get::<Mind>(horse).unwrap().fighting());
    }

    #[test]
    fn a_townsperson_runs_700_from_a_hostile_within_800() {
        let mut rng = Rng::of(50, 39, 10);
        let here = Vec3::ZERO;
        let goblin = Vec3::new(500.0, 0.0, 0.0);
        let far = Vec3::new(900.0, 0.0, 0.0);
        assert_eq!(danger(here, &[far], &mut rng), None);
        for _ in 0..50 {
            let to = danger(here, &[far, goblin], &mut rng).unwrap();
            assert!((to.x + 700.0).abs() <= 100.0 && to.z.abs() <= 100.0, "{to}");
        }
    }

    /// Sector (0, 0) walkable but for a wall across it along the cells' row 10, with a door in it
    /// at cell (10, 10), closed: its cell taken, as its trigger takes it.
    fn walled() -> Heights {
        use opensacred_data::world::{Cell, SIDE, Sector};
        let cell = |ground| Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground,
        };
        let mut cells = vec![cell(0x40); SIDE * SIDE];
        for x in 0..SIDE {
            cells[10 * SIDE + x] = cell(0x41);
        }
        cells[10 * SIDE + 10] = cell(0x40);
        let mut heights = Heights::default();
        heights.add(&Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: Vec::new(),
        });
        heights.occupy((10, 10), None);
        heights
    }

    /// The centre of cell `(x, y)` on the ground.
    fn at_cell(x: f32, y: f32) -> Vec3 {
        crate::iso::ground(x + 0.5, y + 0.5)
    }

    #[test]
    fn a_creature_walks_to_the_side_of_the_door_it_stands_on() {
        let mut heights = walled();
        let door = at_cell(10.0, 10.0);
        // Before the door on either side, the cell beside it on that side, the nearest.
        let cell = |p: Vec3| paths::cell_of(p.x, p.z);
        let near = beside(door, at_cell(10.0, 4.0), None, &heights);
        assert_eq!(cell(near), (10, 9));
        assert_eq!(
            cell(beside(door, at_cell(14.0, 17.0), None, &heights)),
            (11, 11)
        );
        // On a level with no cell beside the door that may be walked on, the door itself.
        let inside = Some((0, 0, 1));
        assert_eq!(beside(door, at_cell(10.0, 4.0), inside, &heights), door);
        // The closed door cuts the way through the wall; open, it leads through it.
        let other_side = spot(at_cell(10.0, 15.0));
        assert!(!way_to(at_cell(10.0, 4.0), None, other_side, &heights));
        assert!(way_to(
            at_cell(30.0, 4.0),
            None,
            spot(at_cell(2.0, 8.0)),
            &heights
        ));
        // A target beyond the window is brought nearer along the line: on this side it is
        // reached, beyond the wall not.
        let far_side = spot(at_cell(10.0, 62.0));
        let far_along = spot(at_cell(60.0, 4.0));
        assert!(way_to(at_cell(1.0, 4.0), None, far_along, &heights));
        assert!(!way_to(at_cell(10.0, 4.0), None, far_side, &heights));
        heights.free((10, 10), None);
        assert!(way_to(at_cell(10.0, 4.0), None, other_side, &heights));
        assert!(way_to(at_cell(10.0, 4.0), None, far_side, &heights));
    }

    #[test]
    fn a_rider_goes_for_a_gate_alone_and_gets_off_before_another_door() {
        // The 19 gates, palisades, fences and cages (#2293).
        let gates = (5600..5800)
            .filter(|&kind| opensacred_rules::minds::rides_through(kind))
            .count();
        assert_eq!(gates, 19);
        let heights = walled();
        let mut world = World::new();
        let way = InTheWay::of(0, 0, None);
        let mind = || Mind::new(Vec3::ZERO, 0, (true, false), None, [70.0, 180.0], (256, 5));
        let mut go = |rides: bool, kind: u32| {
            let rider = world.spawn_empty().id();
            let door = world.spawn_empty().id();
            let mut mind = mind();
            let mut commands = world.commands();
            go_for_door(
                &mut commands,
                (rider, at_cell(14.0, 17.0), &way, rides),
                &mut mind,
                (door, at_cell(10.0, 10.0), None, kind),
                &heights,
            );
            world.flush();
            (
                world.get::<crate::npcs::GetsOff>(rider).is_some(),
                world.get::<Walk>(rider).is_some(),
                mind.opening == Some(door),
            )
        };
        // A rider before a house's door gets off its horse and goes for it not yet.
        assert_eq!(go(true, 5650), (true, false, false));
        // Before a palisade's gate it rides on to it, as one on foot before any door.
        assert_eq!(go(true, 5740), (false, true, true));
        assert_eq!(go(false, 5650), (false, true, true));
    }

    #[test]
    fn a_creature_goes_for_the_doors_point_on_its_side() {
        let heights = walled();
        let door = at_cell(10.0, 10.0);
        let cell = |p: Vec3| paths::cell_of(p.x, p.z);
        // The door of the wall along x at y = 10, its floor's side up the grid's −y (a door of
        // 28°), a creature on the floor taking the inner point and one on the ground the outer.
        let points = DoorPoints::of((10, 10), (1.0, 0.0), |cell, on_floor| {
            on_floor == (cell.1 < 10) && cell.1 != 10
        });
        let inside = Some((0, 0, 1));
        let on_floor = door_point(Some(points), door, at_cell(10.0, 4.0), inside, &heights);
        assert_eq!(cell(on_floor), (10, 9));
        let outside = door_point(Some(points), door, at_cell(14.0, 17.0), None, &heights);
        assert_eq!(cell(outside), (10, 11));
        // Without points, the cell beside the door on its side.
        let beside = door_point(None, door, at_cell(14.0, 17.0), None, &heights);
        assert_eq!(cell(beside), (11, 11));
    }

    #[test]
    fn a_creature_goes_for_a_marked_closed_door_of_its_own_sector() {
        let mut world = World::new();
        let (ours, theirs, open) = (
            world.spawn_empty().id(),
            world.spawn_empty().id(),
            world.spawn_empty().id(),
        );
        let door = |door, at: Vec3, open| DoorSeen {
            door: (door, at, None, 5650),
            at: spot(at),
            layer: 0,
            open,
            barred: false,
            marked: true,
        };
        let here = at_cell(10.0, 20.0);
        // A nearer door beyond the sector's edge, at 64 cells, is not its own; an open one it
        // passes by.
        let doors = [
            door(ours, at_cell(30.0, 20.0), false),
            door(theirs, at_cell(64.0, 20.0), false),
            door(open, at_cell(12.0, 20.0), true),
        ];
        assert_eq!(
            door_in_way(here, 0, &doors).map(|(door, ..)| door),
            Some(ours)
        );
        assert_eq!(
            door_in_way(at_cell(70.0, 20.0), 0, &doors).map(|(door, ..)| door),
            Some(theirs)
        );
        assert_eq!(door_in_way(here, 0, &doors[1..]), None);
    }

    #[test]
    fn a_saved_mind_keeps_its_foe_and_door_by_their_names_and_its_deadline_by_the_clock() {
        use crate::doings::{Moment, Saves};
        use crate::names::{Id, Loading, Named, Names, NamesPlugin, Refs, Saving, mapped};
        let mut saved = App::new();
        saved.add_plugins(NamesPlugin);
        let wolf = saved.world_mut().spawn(Named(Id::Born(3))).id();
        let door = saved.world_mut().spawn(Named(Id::Placed(80))).id();
        let nameless = saved.world_mut().spawn_empty().id();
        let mut mind = Mind::new(Vec3::ZERO, 0x1, (false, true), None, [70.0, 180.0], (76, 2));
        mind.foe = Some(wolf);
        mind.entering = Some((door, Vec3::X, Vec3::Y));
        mind.last = Some(nameless);
        mind.chains = true;
        let mut refs = Refs::default();
        let kept = mapped(
            mind.saved(&Moment { now: 50.0 }).unwrap(),
            &mut Saving {
                world: saved.world(),
                refs: &mut refs,
            },
        );
        let bytes = postcard::to_stdvec(&(&refs, &kept)).unwrap();
        let (refs, kept): (Refs, SavedMind) = postcard::from_bytes(&bytes).unwrap();
        let mut loaded = App::new();
        loaded.add_plugins(NamesPlugin);
        loaded.world_mut().spawn_batch((0..6).map(|_| ()));
        let door = loaded.world_mut().spawn(Named(Id::Placed(80))).id();
        let wolf = loaded.world_mut().spawn(Named(Id::Born(3))).id();
        let kept = mapped(
            kept,
            &mut Loading {
                refs: &refs,
                names: loaded.world().resource::<Names>(),
                lost: 0,
            },
        );
        let back = Mind::again(kept, &Moment { now: 5.0 }).unwrap();
        assert_eq!(back.foe, Some(wolf));
        assert_eq!(back.entering, Some((door, Vec3::X, Vec3::Y)));
        assert_eq!(back.last, None);
        assert!(back.chains);
        // Its deadline of the real clock 45 s earlier, the load 45 s before the save's time.
        let mut thinks = mind.thinks.clone();
        thinks.rebase(-45.0);
        assert_eq!(back.thinks, thinks);
    }

    #[test]
    fn a_mind_keeps_its_home_on_the_ground() {
        let home = Vec3::new(1000.0, 50.0, -300.0);
        let mind = Mind::new(home, 0x1, (true, false), None, [70.0, 140.0], (76, 2));
        assert_eq!(mind.home, Spot::new(1000.0, -300.0));
        assert_eq!(point(mind.home, home.y), home);
    }
}
