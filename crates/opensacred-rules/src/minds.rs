//! The creatures' decisions (docs/re/ai.md, docs/re/fight.md): what a creature does next by its
//! state, its surroundings and the time step. Each is dormant until its own timer or the hero's
//! look wakes it (`0x5375C0`, `0x547070`), and dormant again farther than 1,500 from the hero
//! (`0x542B20`); a hostile creature steps about by the idle action (`0x54EA30`), a group's members
//! together, or comes at the hero and keeps the band of its reach, takes its place about the hero
//! beside the hero's nearest hostile, flees wounded and goes home from too far (`0x4E0300`), or
//! with a ranged weapon keeps 300 to 800 from the hero and shoots (`0x4DBFC0`); without a straight
//! line or a way to the hero it goes for a door that a hero turned and opens it (`0x4D4800`,
//! `0x53A170`); a peaceful animal flees, steps away and strolls about home, a guard
//! keeps its post, and a townsperson and a child roll what they do at each of the hero's looks,
//! by their behaviours (`0x4EF150`); a creature that shares its cell with another parts from it
//! (`0x544890`), and one that walks into the hero's way steps aside (`0x547070`). The hero's
//! attack chooses where it stands and when it strikes (`0x6172C0`).
//! `opensacred-game` (`ai`, `attack`) keeps the creatures' minds as components, the systems that
//! read the world and carry the decisions out, and the world's points; this module is the rules
//! alone, over points of the ground ([`Spot`]).

use std::ops::{Add, Mul, Sub};

use crate::spawning::Rng;

/// A point of the ground in the world's units, the original's (a cell's side is 53.66563,
/// `CreateNPC` `0x482E9D`; docs/02 §5.1), in which the creatures' distances are measured: `x`
/// and `y` along the grid, which the game's world takes as its x and z.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spot {
    pub x: f32,
    pub y: f32,
}

impl Spot {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Its distance from the origin.
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// Its direction, of length 1, or the origin for a point too near it.
    fn unit(self) -> Self {
        let recip = self.length().recip();
        if recip.is_finite() && recip > 0.0 {
            self * recip
        } else {
            Self::default()
        }
    }

    /// Its distances from the origin along each axis.
    fn abs(self) -> Self {
        Self::new(self.x.abs(), self.y.abs())
    }

    /// It turned about the origin by `degrees`, as the game's world turns a point about its
    /// vertical.
    fn turned(self, degrees: f32) -> Self {
        let (sin, cos) = degrees.to_radians().sin_cos();
        Self::new(self.x * cos + self.y * sin, self.y * cos - self.x * sin)
    }
}

impl Add for Spot {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for Spot {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }
}

impl Mul<f32> for Spot {
    type Output = Self;

    fn mul(self, factor: f32) -> Self {
        Self::new(self.x * factor, self.y * factor)
    }
}

/// How often the hero looks about, in seconds (`0x547070`), and how far, in world units.
const LOOK_EVERY: f32 = 0.3;
pub const RANGE: f32 = 1_875.0;
/// How near the hero wakes a hostile creature of bit 0, standing and while it moves.
const WAKES: f32 = 800.0;
const WAKES_MOVING: f32 = 400.0;
/// How far from the hero, along the grid's x and y, an awake creature goes dormant again
/// (`0x542B20`).
const FAR_X: f32 = 1_500.0;
const FAR_Y: f32 = 1_000.0;
/// How far from home along an axis sends a hostile creature home, in one case of six.
const AWAY: f32 = 200.0;
/// A step of the idle action: within 150 along each axis (`rand() % 301 − 150`).
const STEP: i32 = 150;
/// How far a group's member's step strays from its fellow's (`±50`).
const FELLOW: i32 = 50;
/// The periods of the creatures' timers, and the jitter of each firing (`0x5375C0`).
const PERIOD: f32 = 5.0;
const PERIOD_HOSTILE: f32 = 2.5;
const JITTER: f32 = 1.5;
/// The flags whose creatures' own timers wake them: the hostile ones and the townsfolk.
const TIMED: u32 = 0x3 | 0x408;
/// The behaviours' flags: the wild animals, the farm animals, the guards, the townsfolk
/// (`0x4EF150`); those that the game follows.
const WILD: u32 = 0x40;
const WILD_TOO: u32 = 0x80;
const FARM: u32 = 0x10;
const FARM_LOOSE: u32 = 0x20;
const GUARD: u32 = 0x4000;
const TOWNSFOLK: u32 = 0x8;
const CITIZEN: u32 = 0x400;
/// `CreateNPC`'s tag `0x4C`, which the behaviours take as 8 (`0x4EE030`).
const TOWNSFOLK_TOO: u32 = 0x8000;
const TOWNSFOLKS: u32 = TOWNSFOLK | CITIZEN | TOWNSFOLK_TOO | JUGGLER;
const BEHAVIOURS: u32 = WILD | WILD_TOO | FARM | FARM_LOOSE | GUARD | TOWNSFOLKS;
/// How far from the hero, times the camera's zoom, a townsperson goes dormant again
/// (`0x4F0A87`, the float at `0x891028`; the zoom `0x611400`).
const TOWNSFOLK_RANGE: f32 = 600.0;
/// The children's types (`0x43F670`), which always roll 20.
const CHILDREN: [u32; 7] = [229, 230, 231, 702, 703, 704, 705];
/// How far a townsperson follows its partner, how near they meet, farther than which it walks
/// up to it, and how near it steps off to 220 from it (`0x4EF150` with the flag 4,
/// `0x4FC490`).
const FOLLOWS_WITHIN: f32 = 900.0;
const MEETS: (f32, f32) = (80.0, 200.0);
const CATCHES_UP: f32 = 240.0;
const STEPS_OFF: f32 = 220.0;
const STEP_OFF_TRIES: usize = 10;
/// How near a townsperson the second nearest trader is taken rather than the nearest
/// (`0x4EE660`).
const TRADER_WITHIN: f32 = 800.0;
/// The children's play (`0x4F2AD7`): within 800 of home about itself, farther than 1,000 800
/// toward home, else about home, within 400 along each axis; one farther than 349 from the hero
/// within a building follows it, one within 200 of it turns to it, and one now and then runs to
/// 150 from it.
const PLAYS_ABOUT: i32 = 400;
const PLAYS_NEAR: f32 = 800.0;
const PLAYS_FAR: f32 = 1_000.0;
const GATHERS: f32 = 349.0;
const LOOKS_UP: f32 = 200.0;
const RUNS_UP: f32 = 150.0;
/// How far a townsperson in danger runs from its threat without a door to go in by, within 100
/// more along each axis (`0x4F0BDA`, `0x4F0F5E`).
const RUNS_FROM: f32 = 700.0;
const RUNS_ASTRAY: i32 = 100;
/// How far from a door a townsperson walks toward it by stages, how far a stage goes, within
/// 150 more along each axis, and how far it wanders without a door at dusk (`0x4F0F5E`).
const DOOR_FAR: f32 = 1_000.0;
const DOOR_STAGE: f32 = 700.0;
const DOOR_STAGE_ASTRAY: i32 = 150;
const WANDERS: i32 = 400;
/// How near the Daemon puts the townsfolk in danger, while the hero has solved fewer than 25
/// quests (`0x4F0BDA`, the hero's `+0x2CF`; **hypothesis**: the quests solved, none yet).
const DAEMON_NEAR: f32 = 500.0;
/// How far a child runs from the Daemon (`0x539F60`).
const FLEES_DAEMON: f32 = 700.0;
/// The children's lines, of which one near the hero says one at random (`0x4F2AD7`).
const CHILD_LINES: [&str; 35] = [
    "KIND_0_0",
    "KIND_1_2",
    "KIND_2_0",
    "KIND_2_3",
    "KIND_3_1",
    "KIND_3_6",
    "KINDGENERVT_2",
    "KINDGENERVT_4",
    "KINDGENERVT_5",
    "KID01",
    "KID02",
    "KID03",
    "KID04",
    "KID05",
    "KID06",
    "KID07",
    "KID08",
    "KID09",
    "KID10",
    "KID11",
    "KID12",
    "KID13",
    "KID14",
    "KID15",
    "KID16",
    "KID17",
    "KID18",
    "KID19",
    "KID20",
    "KID21",
    "KID22",
    "KID23",
    "KID24",
    "KID25",
    "KID26",
];
/// How near the hero a hostile creature comes at it from, and how far it follows it before it
/// gives up (`0x542B20`, `0x4E1B5F`).
const ATTACKS_WITHIN: f32 = 800.0;
const GIVES_UP: f32 = 2_000.0;
/// The reach of two creatures of the size of most (`0x4DBC60`: 130 by default; **hypothesis**
/// for every type, whose sizes are not read), the band about it a creature keeps (`0x4E0300`),
/// and in how many cases of 11 it comes on at a run.
const REACH: f32 = 130.0;
const BAND: f32 = 15.0;
const RUNS_IN: u32 = 7;
/// How near the hero a wounded creature flees, how far, and in how many frames of one it does
/// (`0x4E0300`); how far from home a creature at full health in the band goes back (the leash,
/// 2,500 or 5,000; **hypothesis**: the nearer).
const FLEES_WITHIN: f32 = 199.0;
const FLEES_FOR: f32 = 700.0;
const FLEES_ONE_IN: u32 = 8;
const LEASH: f32 = 2_500.0;
/// The places about the hero of the second, third and fourth of its list of hostiles
/// (`0x4E224E`): the directions from the hero along the world's `+x`, `−y` and `+y`, at `R − 15`
/// from it, and how near its place a creature's walk there stops (`+0x176` 40).
const PLACES: [Spot; 3] = [
    Spot::new(1.0, 0.0),
    Spot::new(0.0, -1.0),
    Spot::new(0.0, 1.0),
];
const PLACE_WITHIN: f32 = 40.0;
/// How far along each axis the move's second search moves its point (`0x4F5652`).
const SEARCHES_ASTRAY: i32 = 50;
/// How near a door a creature that went for it opens it, standing (`0x53A170`), and how near the
/// door's point its walk there stops (`+0x176` 20).
const OPENS_WITHIN: f32 = 300.0;
pub const AT_THE_DOOR: f32 = 20.0;
/// The archers' band (`0x4DBFC0`): nearer than 300 an archer backs off, up to ten times between
/// its shots, and farther than 800 it comes on, stopping within 450; between, without a straight
/// line to the hero, it goes round to its place turned about the hero by up to 30° either way,
/// stopping within 25; deciding, it shoots from 250 to 650, and moving, anywhere in the band.
const KEEPS: (f32, f32) = (300.0, 800.0);
const BACKS_OFF: u8 = 10;
const COMES_WITHIN: f32 = 450.0;
const GOES_ROUND: u32 = 30;
const GOES_ROUND_WITHIN: f32 = 25.0;
const SHOOTS: (f32, f32) = (250.0, 650.0);
/// How far from the hero a creature of a behaviour goes dormant again (`0x5370B0`, the distance).
pub const BEHAVES_WITHIN: f32 = 1_500.0;
/// The jugglers' types, and the flag they take at their first update, with which a juggler is a
/// townsperson that always rolls 9 and plays its instrument ([`roll`]).
const JUGGLERS: [u32; 4] = [78, 79, 82, 104];
const JUGGLER: u32 = 0x1000_0000;
/// How near a creature other than an animal makes a wild animal flee and a farm animal step
/// away (800 when it attacks, which the game does not follow yet:
/// docs/decisions/2026-10-20-animals.md).
const FLEE: f32 = 300.0;
const SHY: f32 = 200.0;
/// How far from home along an axis brings a wild animal back, within 50 of it.
const ROAMS: f32 = 380.0;
const BACK: i32 = 50;
/// How far about home the animals stroll: the wild within 100, the farm within 150.
const STROLL_WILD: i32 = 100;
const STROLL_FARM: i32 = 150;
/// How far from its post along an axis sends a guard back to it, and how far about it a guard
/// strolls.
const POST: f32 = 90.0;
const STROLL_GUARD: i32 = 200;
/// The chance of a stroll, a frame's 1 in 101 at 30 frames a second, as a rate a second.
const STROLLS_A_SECOND: f32 = 30.0 / 101.0;
/// The chance that an idle creature sharing its cell parts, a frame's 1 in 6, as a rate a second
/// (`0x544890`), and how far about home or itself it goes.
const PARTS_A_SECOND: f32 = 30.0 / 6.0;
const PART: i32 = 100;
/// The flag that keeps a creature from parting (`CreateNPC`'s tag `0x74`).
const STAYS: u32 = 0x800_0000;
/// How near the hero a creature not hostile that walks steps aside, how far about itself it
/// looks for a point it reaches, and in how many tries (`0x547070`).
const ASIDE_WITHIN: f32 = 91.0;
const ASIDE: i32 = 200;
const ASIDE_TRIES: usize = 10;

/// A creature's mind while no foe is near, and in its fight with the hero.
#[derive(Clone, Debug, PartialEq)]
pub struct Mind {
    /// Where it was made, the point it goes home to.
    pub home: Spot,
    /// Its flags word (`+0x1F4`, `ways::flags_word`).
    pub flags: u32,
    /// Made by a sector's `Enter` function (`+0x14` bit `0x80000000`): its steps turn about the
    /// hero and move its home.
    pub enter: bool,
    /// Its group, if it is in one (`+0x249`).
    pub group: Option<u32>,
    /// Its walk speed in world units a second, before the game's speed, and its run speed.
    pub speed: f32,
    pub run: f32,
    /// Its type, and its `Creature.pak` class: the animals (4 and 6) flee from the others.
    pub creature: u32,
    pub class: u8,
    /// Whether it has a dialogue (`+0x245`, `sdk_STAMP_IDX` `0x5498F0`), which holds it: a
    /// townsperson keeps still, a guard does not stroll, a crowded creature does not part.
    pub dialogue: bool,
    /// Whether it is a trader: a smith, a merchant or a wizard (`+0x200` bits `0x1000`, `0x2000`,
    /// `0x4000`, `0x4000000`, `CreateNPC`'s tags `0x23`, `0x24`, `0x25`, `0x64`), which the
    /// townsfolk visit, which does not part and which a meeting does not stop.
    pub trader: bool,
    /// Its level (`+0x3FE`, `fight::level`).
    pub level: u32,
    /// A peaceful creature awake, which takes its behaviour at every frame while it stands.
    pub active: bool,
    /// Whether it holds a ranged weapon in a hand (item category 5, weapon kinds 7, 12, 13 and 14,
    /// `0x427ED0`), with which it fights at range (the AI's state 8, `0x4DBFC0`).
    pub ranged: bool,
    /// Whether it fights the hero (the AI's state 4, `0x4E0300`, or 8), and whether it has fled
    /// in this fight (`+0x1F4` bit `0x200`).
    fighting: bool,
    fled: bool,
    /// In melee: whether it walks to its place about the hero for this frame (the sub-state 5),
    /// and then at a run.
    placing: Option<bool>,
    /// Whether it goes for a door in its way (the AI's state 1 with the door its target,
    /// `0x4D4800`).
    to_door: bool,
    /// At range: whether it decides (the sub-state 0) rather than moves to its band (1), how many
    /// times it has backed off since it last decided (`+0x12B`), whether it moves at a run
    /// (`+0x1F8`), and the chain of its shots (`+0x210`).
    deciding: bool,
    backs: u8,
    runs_at_range: bool,
    shots: Chain,
    awake: bool,
    clock: f32,
    period: f32,
}

impl Mind {
    /// The mind of a creature made at `home` with the flags word `flags`, by an `Enter` function
    /// or not, by `CreateNPC` (`created`) or a fill: `CreateNPC` wakes its hostile creatures as
    /// it makes them (`sdk_WAKE`) and gives them a timer of 2.5 s.
    pub fn new(
        home: Spot,
        flags: u32,
        (enter, created): (bool, bool),
        group: Option<u32>,
        [speed, run]: [f32; 2],
        (creature, class): (u32, u8),
    ) -> Self {
        let hostile = flags & 0x3 != 0;
        // A creature of an `Enter` function still without flags takes them at its first update
        // (`0x5375C0`): the animals of class 6 the loose farm animals', the jugglers theirs, the
        // others the townsfolk's.
        let flags = match (enter, flags, class) {
            (true, 0, 6) => FARM_LOOSE,
            (true, 0, _) if JUGGLERS.contains(&creature) => JUGGLER,
            (true, 0, _) => TOWNSFOLK,
            _ => flags,
        };
        Self {
            home,
            flags,
            enter,
            group,
            speed,
            run,
            creature,
            class,
            dialogue: false,
            trader: false,
            level: 1,
            active: false,
            ranged: false,
            fighting: false,
            fled: false,
            placing: None,
            to_door: false,
            deciding: true,
            backs: 0,
            runs_at_range: false,
            shots: Chain::default(),
            awake: hostile && created,
            clock: 0.0,
            period: if hostile && created {
                PERIOD_HOSTILE
            } else {
                PERIOD
            },
        }
    }

    /// Whether it is hostile to the hero.
    pub fn hostile(&self) -> bool {
        self.flags & 0x3 != 0
    }

    /// Whether it is a farm or a wild animal by its behaviour.
    pub fn is_animal(&self) -> bool {
        self.flags & (WILD | WILD_TOO | FARM | FARM_LOOSE) != 0
    }

    /// Whether it is a townsperson by its behaviour.
    pub fn is_townsperson(&self) -> bool {
        self.flags & TOWNSFOLKS != 0
    }

    /// Whether it is a child, by its type.
    pub fn is_child(&self) -> bool {
        CHILDREN.contains(&self.creature)
    }

    /// How far from the hero it goes dormant again while it stands awake by its behaviour: a
    /// townsperson farther than 600 times the camera's `zoom`, any other farther than 1,500.
    pub fn range(&self, zoom: f32) -> f32 {
        if self.is_townsperson() {
            TOWNSFOLK_RANGE * zoom
        } else {
            BEHAVES_WITHIN
        }
    }

    /// Its own timer, the hero's look and its thought this frame, by what it `senses`
    /// (`0x5375C0`, `0x542B20`): the timer of a hostile creature or a townsperson wakes it
    /// every 5 s (2.5 s for one that `CreateNPC` woke), jittered by ±1.5 s; the hero's look
    /// wakes a creature not hostile within 1,875 and a hostile one of bit 0 within 800 (400
    /// while the hero moves), and gives the idle action to the other hostile ones in range.
    /// Awake, it goes dormant again farther than 1,500 or 1,000 from the hero along the grid's
    /// axes; a hostile creature not fighting walks home in one case of six from farther than
    /// 200, else comes at the hero from within 800 when it takes it on or has it within its
    /// reach and a straight line of walkable cells leads to it (`clear`), raising its group's
    /// alarm in one case of two (`0x46AE70`); without that line one that goes for doors
    /// ([`Self::seeks_doors`]) goes for the door in its way that `door` finds (`0x5442E8`,
    /// `0x4D4800`), and otherwise in one case of three it takes the idle action; a peaceful
    /// creature of a behaviour takes it while it stands. While it goes for a door it thinks
    /// nothing.
    pub fn think(
        &mut self,
        senses: &Senses,
        clear: impl FnOnce() -> bool,
        door: impl FnOnce() -> bool,
        rng: &mut Rng,
    ) -> Thought {
        if self.to_door {
            return Thought::default();
        }
        let Senses {
            dt,
            seen,
            here,
            hero,
            hero_moves,
            takes_on,
            walking,
        } = *senses;
        let mut thought = Thought::default();
        if self.flags & TIMED != 0 {
            self.clock += dt;
            if self.clock >= self.period {
                let jitter = f32::from(u16::try_from(rng.below(31)).unwrap_or(15)) * 0.1;
                self.clock = jitter - JITTER;
                self.awake = true;
            }
        }
        let distance = (here - hero).length();
        let wakes = if hero_moves { WAKES_MOVING } else { WAKES };
        let mut idle = false;
        if seen && distance < RANGE {
            if !self.hostile() || (self.flags & 0x1 != 0 && distance < wakes) {
                self.awake = true;
            } else {
                idle = true;
            }
        }
        if self.awake {
            self.awake = false;
            let away = (here - hero).abs();
            if away.x > FAR_X || away.y > FAR_Y {
                self.clock = -JITTER;
            } else if self.hostile() && !self.fighting {
                let from_home = (here - self.home).abs();
                // The hero worth experience to it, or within its reach, within 800, a straight
                // line of walkable cells to it: it comes at the hero (`0x542B20`).
                let takes_on = takes_on || distance <= REACH;
                if rng.below(6) == 0 && (from_home.x > AWAY || from_home.y > AWAY) {
                    if !walking {
                        thought.home = Some(self.home);
                    }
                } else if takes_on && distance < ATTACKS_WITHIN && clear() {
                    self.fighting = true;
                    self.deciding = true;
                    thought.attacks = true;
                    // In a group, in one case of two, it raises the alarm (`0x46AE70`).
                    thought.alarm = self.group.is_some() && rng.below(2) == 0;
                } else if takes_on && distance < ATTACKS_WITHIN && self.seeks_doors() && door() {
                    // No straight line: the door in its way (`0x5442E8`).
                    self.to_door = true;
                    thought.door = true;
                } else if rng.below(3) == 0 {
                    // Dormant again, by chance stepping about.
                    idle = true;
                }
            } else if self.flags & BEHAVIOURS != 0 {
                self.active = true;
            }
        }
        if idle && !walking && !self.fighting && !self.to_door {
            thought.step = idle_step(self, here, hero, rng);
            if let (Some(step), true) = (thought.step, self.enter) {
                self.home = step;
            }
        }
        thought
    }

    /// The step of a group's member whose fellow, one of an `Enter` function, stepped by `step`:
    /// the same step from `here`, within 50 more along each axis, its home moving with it.
    pub fn fellow_steps(&mut self, here: Spot, step: Spot, rng: &mut Rng) -> Spot {
        let target = about(here + step, FELLOW, rng);
        self.home = target;
        target
    }

    /// The alarm of the group `group` (`0x46AE70`): it sends the group's hostile members at the
    /// hero.
    pub fn hears_alarm(&mut self, group: u32) {
        if self.group == Some(group) && self.hostile() && !self.fighting && !self.to_door {
            self.fighting = true;
            self.deciding = true;
        }
    }

    /// Whether it goes for a door in its way (`0x4D4800`): not of the classes 2, 5 and 6
    /// (`Creature.pak +0x04`; by their models the beasts, the undead and the animals). The
    /// original lets one of class 5 whose `+0x25F` is set go too, a field that the game does not
    /// keep.
    pub fn seeks_doors(&self) -> bool {
        !matches!(self.class, 2 | 5 | 6)
    }

    /// Whether it goes for a door in its way (the AI's state 1).
    pub fn to_door(&self) -> bool {
        self.to_door
    }

    /// Standing by the door it went for, which it opened or could not, it takes the AI's state 0
    /// again (`0x53A170`), whose choice looks for the hero anew (`0x542B20`), at once
    /// (**hypothesis**: the sub-state that the state 0 takes then is not read).
    pub fn left_door(&mut self) {
        self.to_door = false;
        self.awake = true;
    }

    /// It leaves its fight with the hero.
    fn leaves_fight(&mut self) {
        self.fighting = false;
        self.fled = false;
        self.placing = None;
    }

    /// What a creature that fights the hero does next, by what it sees (`0x4E0300`): farther
    /// than 2,000 it gives up; wounded to a third, near the hero, it flees 700 from it at a run,
    /// once a fight (`flees`); farther than the band of its reach it comes on, at a run in 7
    /// cases of 11, after the hero as it moves; nearer, it steps back; in the band at full
    /// health, farther than 2,500 from home, it goes home and leaves the fight; else it stands
    /// in the band, turned to the hero, where it strikes (#280), and decides again when the
    /// strike ends (`0x4E2147`). Deciding, the second, third or fourth of the hero's list of
    /// hostiles that stands nearer than `R − 15` to the first takes its place about the hero
    /// instead ([`place`], the sub-state 5): farther than the band from it, it walks there at a
    /// run in one case of two (never one of class 5), stopping within 40, for a frame, and is
    /// then steered by the hero alone, striking in the band, coming on beyond it and stepping
    /// back nearer. One with a ranged weapon fights at range instead ([`Self::at_range`]), where
    /// a straight line of walkable cells to the hero (`clear`) counts. `None` while it does not
    /// fight, or walks on.
    pub fn pursue(
        &mut self,
        sight: &Pursuit,
        clear: impl FnOnce() -> bool,
        rng: &mut Rng,
    ) -> Option<Pursues> {
        if !self.fighting {
            return None;
        }
        if self.ranged {
            return self.at_range(sight, clear, rng);
        }
        let Pursuit {
            here,
            hero,
            walk,
            striking,
            health,
            hero_strikes,
            listed,
        } = *sight;
        let (lo, hi) = (REACH - BAND, REACH + BAND);
        let within = lo + BAND;
        let d = (hero - here).length();
        if d > GIVES_UP {
            self.leaves_fight();
            return Some(Pursues::GivesUp);
        }
        let steps_back = || Pursues::StepsBack(here + (here - hero).unit() * ((lo + hi) / 2.0));
        // Its place held for a frame, it is steered by the hero alone, as the sub-state 2 steers
        // a creature that moves (`0x4E23A4`).
        if let Some(runs) = self.placing.take() {
            return Some(if d > hi {
                Pursues::ComesOn { runs, within }
            } else if d < lo {
                steps_back()
            } else {
                Pursues::Strikes((!striking).then(|| rng.below(5)))
            });
        }
        let roll = rng.below(FLEES_ONE_IN);
        if let Some(health) = health
            && !striking
            && flees(health, d, self, hero_strikes, roll)
        {
            self.fled = true;
            return Some(Pursues::Flees(here + (here - hero).unit() * FLEES_FOR));
        }
        // Deciding, near the hero's nearest hostile, it takes its place (`0x4E1404`, `0x4E224E`).
        if walk.is_none()
            && !striking
            && let Some(to) = listed.and_then(|listed| place(here, hero, listed))
        {
            let runs = self.class != 5 && rng.below(2) == 0;
            if (to - here).length() > hi {
                self.placing = Some(runs);
                return Some(Pursues::TakesPlace {
                    to,
                    runs,
                    within: PLACE_WITHIN,
                });
            }
        }
        match walk {
            // Coming on, it follows the hero as it moves; any other walk it ends first (#387).
            Some(walk) if walk.comes_on && (walk.target - hero).length() > BAND => {
                Some(Pursues::ComesOn {
                    runs: walk.runs,
                    within,
                })
            }
            Some(_) => None,
            None if d > hi => Some(Pursues::ComesOn {
                runs: rng.below(11) < RUNS_IN,
                within,
            }),
            None if d < lo => Some(steps_back()),
            // In the band at full health and far from home, it goes home (the leash).
            None if health.is_some_and(|(current, most)| current >= most)
                && (here - self.home).length() > LEASH =>
            {
                self.leaves_fight();
                Some(Pursues::GoesBack)
            }
            None => Some(Pursues::Strikes((!striking).then(|| rng.below(5)))),
        }
    }

    /// What a creature fighting in melee does that comes on at the hero at `hero` with no
    /// straight line to it: as the band's sub-state finds no way to the hero (`0x4E4931`) and the
    /// move finds none to its point twice, the second time to the point moved by up to 50 along
    /// each axis (`0x4F4770` at `0x4F5652`), it comes on while a way leads to either (`reaches`);
    /// else one that goes for doors ([`Self::seeks_doors`]) goes for the door in its way that
    /// `door` finds (`0x4D4800`), leaving the fight; else it gives the fight up.
    pub fn blocked(
        &mut self,
        hero: Spot,
        mut reaches: impl FnMut(Spot) -> bool,
        door: impl FnOnce() -> bool,
        rng: &mut Rng,
    ) -> Blocked {
        if reaches(hero) || reaches(about(hero, SEARCHES_ASTRAY, rng)) {
            return Blocked::ComesOn;
        }
        self.leaves_fight();
        if self.seeks_doors() && door() {
            self.to_door = true;
            Blocked::Door
        } else {
            Blocked::GivesUp
        }
    }

    /// What a creature with a ranged weapon that fights the hero does next, by what it sees and
    /// whether a straight line leads to the hero (`clear`), as `0x4DBFC0` keeps it 300 to 800
    /// from the hero: while its shot plays it waits; deciding, it shoots from 250 to 650, else it
    /// moves to its band; moving, it gives up farther than 2,000; nearer than 300, up to ten times
    /// since it last decided and not fled, it backs off once it stands, as far again from the
    /// hero, at a walk in one case of two, else at a run (a walk for one of class 5); farther than
    /// 800 it comes on, after the hero as it moves, stopping within 450; between, without a
    /// straight line, it goes round once it stands to its place turned about the hero by −30° to
    /// +30°, stopping within 25; else it shoots, whether it walks or not. A shot is one of its
    /// attack's clips (one of five), each 0.1 faster than the last up to 1.7 times (`+0x210`,
    /// which no other action of the game's archers breaks), and it decides again after it.
    fn at_range(
        &mut self,
        sight: &Pursuit,
        clear: impl FnOnce() -> bool,
        rng: &mut Rng,
    ) -> Option<Pursues> {
        let Pursuit {
            here,
            hero,
            walk,
            striking,
            ..
        } = *sight;
        // The shot's action plays while its AI is idle (the AI's state 0, `0x4DC9A3`).
        if striking {
            return None;
        }
        let d = (hero - here).length();
        if self.deciding {
            self.backs = 0;
            if (SHOOTS.0..=SHOOTS.1).contains(&d) {
                return Some(self.shoot(rng));
            }
            self.deciding = false;
        }
        if d > GIVES_UP {
            self.leaves_fight();
            return Some(Pursues::GivesUp);
        }
        if d < KEEPS.0 && !self.fled && self.backs < BACKS_OFF {
            if walk.is_some() {
                return None;
            }
            self.backs += 1;
            self.runs_at_range = rng.below(2) == 0 && self.class != 5;
            return Some(Pursues::BacksOff {
                to: here + (here - hero),
                runs: self.runs_at_range,
            });
        }
        if d > KEEPS.1 {
            return match walk {
                Some(walk) if walk.comes_on && (walk.target - hero).length() <= BAND => None,
                _ => Some(Pursues::ComesOn {
                    runs: self.runs_at_range,
                    within: COMES_WITHIN,
                }),
            };
        }
        if d >= KEEPS.0 && !clear() {
            if walk.is_some() {
                return None;
            }
            let turn = f32::from(u16::try_from(rng.below(2 * GOES_ROUND + 1)).ok()?)
                - f32::from(u16::try_from(GOES_ROUND).ok()?);
            return Some(Pursues::GoesRound {
                to: hero + (here - hero).turned(turn),
                runs: self.runs_at_range,
                within: GOES_ROUND_WITHIN,
            });
        }
        Some(self.shoot(rng))
    }

    /// A shot (the action 5, `0x4DCB80`): the choice of its attack's clip and the factor of its
    /// speed by its chain; it decides again after it.
    fn shoot(&mut self, rng: &mut Rng) -> Pursues {
        self.deciding = true;
        self.backs = 0;
        Pursues::Shoots {
            choice: rng.below(5),
            chain: self.shots.strike(),
        }
    }

    /// Where it parts to, standing at `here`, `crowded` by another in its cell, in a frame of `dt`
    /// seconds (`0x544890`): unless it stays (`CreateNPC`'s tag `0x74`), has a dialogue, is a
    /// trader or fights, at a rate of 5 a second, to a point by [`parting`].
    pub fn parts(&self, here: Spot, crowded: bool, dt: f32, rng: &mut Rng) -> Option<Spot> {
        (crowded
            && self.flags & STAYS == 0
            && !self.dialogue
            && !self.trader
            && !self.fighting
            && chance(rng, PARTS_A_SECOND, dt))
        .then(|| parting(self.home, here, rng))
    }

    /// What it does now by its behaviour, awake and standing at `here`, `crowded` or not, in a
    /// frame of `dt` seconds, the nearest it fears at `fear` (`behaviour`): an animal strolls
    /// at a rate of 30 in 101 a second, or at once where another shares its cell, and a guard at
    /// the same rate.
    pub fn behaves(
        &self,
        here: Spot,
        fear: Option<Spot>,
        crowded: bool,
        dt: f32,
        rng: &mut Rng,
    ) -> Option<(Spot, Why)> {
        let strolls = chance(rng, STROLLS_A_SECOND, dt) || (crowded && self.is_animal());
        behaviour(self, here, fear, strolls, rng)
    }

    /// Where it steps aside to, walking at `here`, at the hero's look from `hero` (`0x547070`):
    /// not hostile and within 91 of the hero, to the first of 10 points within 200 of itself that
    /// it `reaches`.
    pub fn steps_aside(
        &self,
        here: Spot,
        hero: Spot,
        rng: &mut Rng,
        mut reaches: impl FnMut(Spot) -> bool,
    ) -> Option<Spot> {
        if self.hostile() || (here - hero).length() >= ASIDE_WITHIN {
            return None;
        }
        (0..ASIDE_TRIES)
            .map(|_| about(here, ASIDE, rng))
            .find(|&to| reaches(to))
    }

    /// Whether it turns to the partner it meets and talks: an adult, in one case of two.
    pub fn talks_at_meeting(&self, rng: &mut Rng) -> bool {
        !self.is_child() && rng.below(2) == 0
    }

    /// A juggler's instrument, if its hands are empty, at random (`0x4F29E2`), and whether it
    /// plays on and on: one of the flag `0x10000000`.
    pub fn juggles(&self, rng: &mut Rng) -> (usize, bool) {
        let instrument = usize::try_from(rng.below(4)).unwrap_or(0);
        (instrument, self.flags & JUGGLER != 0)
    }
}

/// The flags the creature maker (`0x480D40`) gives a peaceful creature of type `creature` made
/// without flags, as a sector's fill makes them: the wild animals' to the wolves, the crows, the
/// bats, the first rabbit, the deer, the bears, the bulls and the wild boars; the farm animals'
/// to the pigs, the cows, the sheep, the horses and the chickens; none to the others, the two
/// other rabbits among them. `CreateNPC` writes its own flags over them (`0x4839F6`).
pub fn of_type(creature: u32) -> u32 {
    const WILD_TYPES: [u32; 25] = [
        512, 513, 514, 515, 516, 530, 531, 532, 533, 536, 537, 538, 540, 541, 543, 545, 567, 568,
        569, 570, 589, 590, 594, 595, 596,
    ];
    const FARM_TYPES: [u32; 25] = [
        520, 521, 522, 523, 524, 525, 526, 527, 528, 544, 550, 551, 552, 553, 554, 555, 556, 557,
        558, 560, 561, 562, 563, 565, 566,
    ];
    if WILD_TYPES.contains(&creature) {
        WILD
    } else if FARM_TYPES.contains(&creature) {
        FARM
    } else {
        0
    }
}

/// The hero's look about, every 0.3 s (`0x547070`): the time since it last looked.
#[derive(Clone, Copy, Debug, Default)]
pub struct Look(f32);

impl Look {
    /// Whether the hero looks in a frame of `dt` seconds.
    pub fn looks(&mut self, dt: f32) -> bool {
        self.0 += dt;
        let looks = self.0 >= LOOK_EVERY;
        if looks {
            self.0 = 0.0;
        }
        looks
    }
}

/// What a creature senses as it thinks ([`Mind::think`]): the time step in seconds; whether the
/// hero looked in this frame with the creature in the 3 × 3 sectors about it; where the creature
/// and the hero stand; whether the hero moves; whether the creature takes on the hero by the test
/// of the levels (`fight::takes_on`); and whether it walks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Senses {
    pub dt: f32,
    pub seen: bool,
    pub here: Spot,
    pub hero: Spot,
    pub hero_moves: bool,
    pub takes_on: bool,
    pub walking: bool,
}

/// What a creature's thought makes it do this frame ([`Mind::think`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Thought {
    /// It walks home, to this point.
    pub home: Option<Spot>,
    /// It comes at the hero (the AI's state 4), and raises its group's alarm.
    pub attacks: bool,
    pub alarm: bool,
    /// The idle action's step: the point it walks to, its home for one of an `Enter` function.
    pub step: Option<Spot>,
    /// It goes for the door in its way that its thought found (the AI's state 1).
    pub door: bool,
}

/// What the idle action makes a creature at `here` do: in one case of ten nothing yet (the
/// original fidgets), else in one case of four a step: within 150 along each axis, or home in one
/// case of four, for a creature of an `Init` function or the start script; its place turned about
/// the hero by −30° to +30°, for one of an `Enter` function; `None` otherwise.
fn idle_step(mind: &Mind, here: Spot, hero: Spot, rng: &mut Rng) -> Option<Spot> {
    if rng.below(10) == 0 || rng.below(4) != 0 {
        return None;
    }
    if mind.enter {
        let degrees = f32::from(u16::try_from(rng.below(61)).ok()?) - 30.0;
        return Some(hero + (here - hero).turned(degrees));
    }
    if rng.below(4) == 0 {
        return Some(mind.home);
    }
    Some(about(here, STEP, rng))
}

/// What a creature fighting the hero sees ([`Mind::pursue`]): where it and the hero stand, its
/// walk if it walks, whether it strikes, its health and its most if it has one, whether the hero
/// strikes, and where it stands in the hero's list of hostiles if it is in it (`0xAD3CE0`,
/// nearest first, docs/re/motions.md, "The fight stance"): its index, 0 the nearest, and where
/// the nearest stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pursuit {
    pub here: Spot,
    pub hero: Spot,
    pub walk: Option<Walking>,
    pub striking: bool,
    pub health: Option<(u32, u32)>,
    pub hero_strikes: bool,
    pub listed: Option<(usize, Spot)>,
}

/// The place about the hero at `hero` of a creature at `here` that is the `index`-th of the
/// hero's list of hostiles, from 0, whose first stands at `first` (`0x4E1404`, `0x4E224E`): for
/// the second, third and fourth, nearer than `R − 15` to the first, the point `R − 15` from the
/// hero along the world's `+x`, `−y` and `+y`, the grid's axes; `None` for any other.
pub fn place(here: Spot, hero: Spot, (index, first): (usize, Spot)) -> Option<Spot> {
    let along = *PLACES.get(index.checked_sub(1)?)?;
    ((here - first).length() < REACH - BAND).then(|| hero + along * (REACH - BAND))
}

/// A creature's walk: its target, whether it comes on at the hero (a walk of
/// [`Pursues::ComesOn`], which follows the hero as it moves) rather than fleeing, stepping back
/// or going about, and whether it runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Walking {
    pub target: Spot,
    pub comes_on: bool,
    pub runs: bool,
}

/// What a creature fighting the hero does next ([`Mind::pursue`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pursues {
    /// Too far from the hero, it leaves the fight.
    GivesUp,
    /// Wounded, it runs to the point.
    Flees(Spot),
    /// It comes on at the hero, at a run or not, and stops within `within` of it.
    ComesOn { runs: bool, within: f32 },
    /// Too near the hero, it steps back to the point.
    StepsBack(Spot),
    /// Too far from home, it walks home and leaves the fight.
    GoesBack,
    /// In the band, turned to the hero, it strikes by the choice of its attack's clip (one of
    /// five), unless it strikes already.
    Strikes(Option<u32>),
    /// At range, too near the hero, it backs off to the point, at a run or not.
    BacksOff { to: Spot, runs: bool },
    /// At range, without a straight line to the hero, it goes round to the point, at a run or
    /// not, stopping within `within` of it.
    GoesRound { to: Spot, runs: bool, within: f32 },
    /// At range, it stops, turns to the hero and shoots by the choice of its attack's clip (one
    /// of five), at its attack speed times `chain`.
    Shoots { choice: u32, chain: f32 },
    /// In melee, it walks to its place about the hero for a frame, at a run or not, stopping
    /// within `within` of it.
    TakesPlace { to: Spot, runs: bool, within: f32 },
}

/// What a creature fighting in melee does that comes on at the hero without a straight line to
/// it ([`Mind::blocked`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blocked {
    /// A way leads to the hero: it comes on.
    ComesOn,
    /// It leaves the fight for the door in its way.
    Door,
    /// No way, no door: it leaves the fight.
    GivesUp,
}

/// A door as a creature in its way sees it (`0x4D4800`): the door, where it stands, its layer (0
/// the ground, else a building's floor), whether it is open (its trigger's bit 0), barred (bit
/// 15, which keeps it from opening) and marked by a hero (its flags' bit `0x2000`, which the
/// door's handler sets at a hero's turn, `0x44BCD0`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DoorSeen<T> {
    pub door: T,
    pub at: Spot,
    pub layer: i32,
    pub open: bool,
    pub barred: bool,
    pub marked: bool,
}

/// Whether a creature's layer and a door's meet (`0x4D4800`, `0x53A170`): the same, or the ground
/// and the first floor either way.
pub fn meets(creature: i32, door: i32) -> bool {
    creature == door || matches!((creature, door), (0 | 1, 0 | 1))
}

/// The door in the way of a creature at `here` on `layer`, of its sector's `doors` (`0x4D4800`):
/// the nearest, straight from it, that is closed, not barred and marked by a hero, on its layer
/// or on the ground and the first floor either way; so a creature goes only for a closed door
/// that a hero has opened or closed before.
pub fn door_in_the_way<T>(
    here: Spot,
    layer: i32,
    doors: impl IntoIterator<Item = DoorSeen<T>>,
) -> Option<T> {
    doors
        .into_iter()
        .filter(|door| !door.open && !door.barred && door.marked && meets(layer, door.layer))
        .map(|door| ((door.at - here).length(), door.door))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, door)| door)
}

/// Whether a creature that went for a door, standing at `here` on `layer`, opens it, at `door`
/// on `door_layer` (`0x53A170`): within 300 of it, on its layer or the ground and the first floor
/// either way.
pub fn opens(here: Spot, layer: i32, door: Spot, door_layer: i32) -> bool {
    (door - here).length() <= OPENS_WITHIN && meets(layer, door_layer)
}

/// Whether a creature fighting the hero flees (`0x4E0300`): at a third of its `health`'s most or
/// less, within 199 of the hero, not of the classes 1 and 5, not fled yet in this fight, while
/// the hero does not strike, in one case of eight a frame (`roll` of 0 to 7).
fn flees(
    (current, most): (u32, u32),
    distance: f32,
    mind: &Mind,
    hero_strikes: bool,
    roll: u32,
) -> bool {
    current * 3 <= most
        && distance <= FLEES_WITHIN
        && !matches!(mind.class, 1 | 5)
        && !mind.fled
        && !hero_strikes
        && roll == 0
}

/// Whether a chance of `rate` a second comes in a frame of `dt` seconds.
fn chance(rng: &mut Rng, rate: f32, dt: f32) -> bool {
    let p = 1.0 - (-rate * dt).exp();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a probability between 0 and 1, in parts of 10,000"
    )]
    let parts = (p.clamp(0.0, 1.0) * 10_000.0) as u32;
    rng.below(10_000) < parts
}

/// A point within `within` of `at` along each axis, at random.
fn about(at: Spot, within: i32, rng: &mut Rng) -> Spot {
    let mut offset = || {
        let span = u32::try_from(2 * within + 1).unwrap_or(1);
        let d = i32::try_from(rng.below(span)).unwrap_or(within) - within;
        f32::from(i16::try_from(d).unwrap_or(0))
    };
    at + Spot::new(offset(), offset())
}

/// Why a creature walks by itself: it flees at a run, steps away, goes back home or to its post,
/// strolls, parts from another in its cell, or steps out of the hero's way; a townsperson visits
/// an animal, goes home, takes a partner, follows it, steps off from it or meets it, trades,
/// plays, gathers about another at play, goes in or out by a door, wanders, is put into a house
/// or juggles; a hostile creature attacks the hero or gives up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    Flees,
    StepsAway,
    GoesBack,
    Strolls,
    Parts,
    StepsAside,
    Visits,
    GoesHome,
    TakesPartner,
    Follows,
    StepsOff,
    Meets,
    Trades,
    Plays,
    Gathers,
    GoesIn,
    GoesOut,
    Wanders,
    PutIn,
    Juggles,
    Attacks,
    GivesUp,
}

/// A townsperson's roll of 26 (`0x4F22F0`): a child always rolls 20, a juggler of `0x10000000`
/// always 9, one of the jugglers' types 9 in one case of six, and anyone else's 9 counts as 10.
pub fn roll(mind: &Mind, rng: &mut Rng) -> u32 {
    if mind.is_child() {
        return 20;
    }
    let roll = rng.below(26);
    if mind.flags & JUGGLER != 0 || (JUGGLERS.contains(&mind.creature) && rng.below(6) == 0) {
        9
    } else if roll == 9 {
        10
    } else {
        roll
    }
}

/// What a townsperson decides by day, by its roll (`0x4F22F0`), about the creatures `T` near it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision<T> {
    /// It walks to the animal, which it does not go to next.
    Visits(T),
    /// It walks home.
    GoesHome,
    /// It takes the human for its partner, and walks home if `true`.
    Partner(T, bool),
    /// It walks to the trader and talks there.
    Trade(T),
    /// A child's play ([`child`]).
    Child,
    /// It goes into the nearest house.
    House,
    /// A juggler plays its instrument.
    Juggle,
    Stop,
}

/// What a townsperson standing does when it decides by day, by its roll (`0x4F22F0`): 0 takes
/// the nearest human, `human`, for its partner, 2 too and walks home, 1 walks to the nearest
/// animal, `animal`, 3 walks home, 5 and 8 visit the trader `trader`, one of 8 in one case of
/// three only (in the other two its field's branch finds no field, `0x4F2643`); 4, 6 and 7 go
/// into a house (#248), 9 plays an instrument (#253) and a child's 20 its play (#252); the rest
/// stop.
pub fn townsperson<T>(
    mind: &Mind,
    human: Option<T>,
    animal: Option<T>,
    trader: Option<T>,
    rng: &mut Rng,
) -> Decision<T> {
    match (roll(mind, rng), human, animal) {
        (0, Some(human), _) => Decision::Partner(human, false),
        (2, Some(human), _) => Decision::Partner(human, true),
        (1, _, Some(animal)) => Decision::Visits(animal),
        (2 | 3, ..) => Decision::GoesHome,
        (4 | 6 | 7, ..) => Decision::House,
        (9, ..) => Decision::Juggle,
        // An adult's 20 stops (`0x4F231D`).
        (20, ..) if mind.is_child() => Decision::Child,
        (5 | 8, ..) => {
            let eight = mind.flags & (TOWNSFOLK | TOWNSFOLK_TOO) != 0;
            match trader {
                Some(trader) if !eight || rng.below(3) == 0 => Decision::Trade(trader),
                _ => Decision::Stop,
            }
        }
        _ => Decision::Stop,
    }
}

/// The trader a townsperson at `here` visits among `traders`, each at its point (`0x4EE660`): the
/// second nearest when it is within 800, else the nearest.
pub fn trader_for<T: Copy>(here: Spot, mut traders: Vec<(T, Spot)>) -> Option<T> {
    let d = |at: Spot| (at - here).length();
    traders.sort_by(|a, b| d(a.1).total_cmp(&d(b.1)));
    match traders.as_slice() {
        [_, (second, at), ..] if d(*at) <= TRADER_WITHIN => Some(*second),
        [(nearest, _), ..] => Some(*nearest),
        [] => None,
    }
}

/// What a creature standing at `here` does about its partner at `partner`, the hero or not
/// (`0x4EF150` with the flag 4, `0x4FC490`): farther than 900 it forgets it, but never the hero,
/// a player, which it follows; between 80 and 200 it meets it and forgets it; farther than 240 it
/// walks up to it; within 80 it steps off to 220 from it ([`steps_off`]); else it forgets it.
pub fn follow(here: Spot, partner: Spot, hero: bool) -> Why {
    let d = (partner - here).length();
    if d > FOLLOWS_WITHIN {
        if hero { Why::Follows } else { Why::GoesBack }
    } else if d > MEETS.0 && d < MEETS.1 {
        Why::Meets
    } else if d > CATCHES_UP {
        Why::Follows
    } else if d <= MEETS.0 {
        Why::StepsOff
    } else {
        Why::GoesBack
    }
}

/// Where a creature steps off to from its partner at `partner`: the first of 10 points 220 from
/// it at random that it may stand on (`free`).
pub fn steps_off(partner: Spot, rng: &mut Rng, mut free: impl FnMut(Spot) -> bool) -> Option<Spot> {
    (0..STEP_OFF_TRIES)
        .map(|_| {
            let degrees = f32::from(u16::try_from(rng.below(360)).unwrap_or(0));
            partner + Spot::new(STEPS_OFF, 0.0).turned(degrees)
        })
        .find(|&to| free(to))
}

/// What a child does by its roll of 20 (`0x4F2AD7`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Child {
    /// It runs to the point, and its playmates follow it.
    Plays(Spot),
    /// It follows the hero at a run.
    FollowsHero,
    /// It turns to the hero and stands.
    LooksUp,
    /// It runs to the point by the hero.
    RunsUp(Spot),
    /// It runs from the Daemon to the point.
    Flees(Spot),
    /// It turns to the hero and says the line.
    Speaks(&'static str),
}

/// What a child at `here` does by its roll, the hero at `hero` the Daemon or not, and on a
/// settlement's ground (`in_settlement`) or not (`0x4F2AD7`): before the Daemon it runs 700 away
/// from it, within 400 more (`0x539F60`); in one case of 13, or with the hero off a settlement's
/// ground, it plays: within 800 of home a point within 400 of itself, farther than 1,000 a point
/// 800 toward home, else one within 400 of home; else it follows the hero from farther than 349,
/// and nearer, within 200 in one case of nine, turns to it and says one of its 35 lines at
/// random, else turns to it, but in one case of 15 runs to a point 150 from it at random.
pub fn child(
    mind: &Mind,
    here: Spot,
    (hero, daemon): (Spot, bool),
    in_settlement: bool,
    rng: &mut Rng,
) -> Child {
    if daemon {
        let away = (here - hero).unit();
        return Child::Flees(about(here + away * FLEES_DAEMON, WANDERS, rng));
    }
    if rng.below(13) == 0 || !in_settlement {
        let from_home = (mind.home - here).length();
        let to = if from_home <= PLAYS_NEAR {
            about(here, PLAYS_ABOUT, rng)
        } else if from_home > PLAYS_FAR {
            here + (mind.home - here).unit() * PLAYS_NEAR
        } else {
            about(mind.home, PLAYS_ABOUT, rng)
        };
        return Child::Plays(to);
    }
    let d = (hero - here).length();
    if d > GATHERS {
        Child::FollowsHero
    } else if rng.below(9) == 0 && d < LOOKS_UP {
        let index = usize::try_from(rng.below(35)).unwrap_or(0);
        Child::Speaks(CHILD_LINES[index % CHILD_LINES.len()])
    } else if rng.below(15) != 0 {
        Child::LooksUp
    } else {
        let degrees = f32::from(u16::try_from(rng.below(360)).unwrap_or(0));
        Child::RunsUp(hero + Spot::new(RUNS_UP, 0.0).turned(degrees))
    }
}

/// Where a townsperson at `here` in danger runs from its threat at `threat` (`0x4F0BDA`):
/// without a door to go in by, 700 away from it along the line, within 100 more along each axis
/// (`0x4F0F5E`).
pub fn runs_from(here: Spot, threat: Spot, rng: &mut Rng) -> Spot {
    let away = (here - threat).unit();
    about(here + away * RUNS_FROM, RUNS_ASTRAY, rng)
}

/// Whether the hero at `hero`, the Daemon or not, puts a townsperson at `here` in danger: within
/// 500 of it (`0x4F0BDA`).
pub fn daemon_near(daemon: bool, here: Spot, hero: Spot) -> bool {
    daemon && (hero - here).length() < DAEMON_NEAR
}

/// The line of a townsperson's fear before the Daemon, in one case of three of its looks: one of
/// `DaeAh1` to `6` (`0x4F0DAC`).
pub fn daemon_fear(rng: &mut Rng) -> Option<String> {
    (rng.below(3) == 0).then(|| format!("DaeAh{}", 1 + rng.below(6)))
}

/// The evening's way of a townsperson on a free cell ([`evening_way`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Evening {
    /// Indoors, it walks among the building's cells.
    Indoors,
    /// It goes into the house of its door.
    GoesIn,
    /// It walks toward its door, to the point.
    Toward(Spot),
    /// Without a door, it wanders to the point.
    Wanders(Spot),
}

/// The evening's way of a townsperson at `here`, `inside` a building or not, the nearest door
/// at `door` (`0x4F0F5E`), which it takes at dusk and by night, or with the Daemon near:
/// indoors it walks among the building's cells; outdoors it goes into the house of the door, by
/// stages of 700 toward it, within 150 more, from farther than 1,000, and without a door it
/// wanders within 400 of itself.
pub fn evening_way(here: Spot, inside: bool, door: Option<Spot>, rng: &mut Rng) -> Evening {
    match door {
        _ if inside => Evening::Indoors,
        Some(door) if (door - here).length() > DOOR_FAR => {
            let toward = (door - here).unit();
            Evening::Toward(about(here + toward * DOOR_STAGE, DOOR_STAGE_ASTRAY, rng))
        }
        Some(_) => Evening::GoesIn,
        None => Evening::Wanders(about(here, WANDERS, rng)),
    }
}

/// Where a creature at `here` that shares its cell parts to (`0x544890`): within 100 of its home
/// when the original's measure of its distance from home lies between 30 and 400, else within
/// 100 of itself. The measure takes, of each point, its y from its x (`0x545B30`), not the home
/// from the creature, and gives up past ±32,000, so that a creature parts about itself but near
/// the world's diagonal.
pub fn parting(home: Spot, here: Spot, rng: &mut Rng) -> Spot {
    let (a, b) = (here.x - here.y, home.x - home.y);
    let measure = (a * a + b * b).sqrt();
    let measured = a.abs() <= 32_000.0 && b.abs() <= 32_000.0;
    if measured && measure > 30.0 && measure < 400.0 {
        about(home, PART, rng)
    } else {
        about(here, PART, rng)
    }
}

/// A number between `low` and `high`, at random.
fn between(low: f32, high: f32, rng: &mut Rng) -> f32 {
    let part = f32::from(u16::try_from(rng.below(10_001)).unwrap_or(0)) / 10_000.0;
    low + (high - low) * part
}

/// Where a wild animal at `here` runs from `fear` (`0x4EF150`): along each axis of the grid away
/// from it by half to twice its distance along that axis, and across by up to half its distance
/// along the other either way.
fn flight(here: Spot, fear: Spot, rng: &mut Rng) -> Spot {
    let d = here - fear;
    let (x, y) = (d.x.abs(), d.y.abs());
    let along = |offset: f32, rng: &mut Rng| {
        let away = between(offset.abs() / 2.0, offset.abs() * 2.0, rng);
        if offset < 0.0 { -away } else { away }
    };
    let dx = along(d.x, rng) + between(-y / 2.0, y / 2.0, rng);
    let dy = along(d.y, rng) + between(-x / 2.0, x / 2.0, rng);
    here + Spot::new(dx, dy)
}

/// What a peaceful creature awake does now by its behaviour, standing at `here` (`0x4EF150`): a
/// wild animal (flags `0x40`, `0x80`) runs from the nearest creature other than an animal within
/// 300 of it, `fear`, the hero among them ([`flight`]), walks back within 50 of home from farther
/// than 380 along an axis, and strolls within 100 of home by chance; a farm animal (`0x10`,
/// `0x20`) steps away from such a creature within 200, to the point beyond itself, and strolls
/// within 150 of home; a guard (`0x4000`) strolls within 200 of its post by chance, else walks
/// back to it from farther than 90 along an axis. The walk's target and why it walks.
fn behaviour(
    mind: &Mind,
    here: Spot,
    fear: Option<Spot>,
    strolls: bool,
    rng: &mut Rng,
) -> Option<(Spot, Why)> {
    let near = |within: f32| fear.filter(|f| (*f - here).length() < within);
    if mind.flags & (WILD | WILD_TOO) != 0 {
        if let Some(fear) = near(FLEE) {
            return Some((flight(here, fear, rng), Why::Flees));
        }
        let from_home = (here - mind.home).abs();
        if from_home.x > ROAMS || from_home.y > ROAMS {
            return Some((about(mind.home, BACK, rng), Why::GoesBack));
        }
        return strolls.then(|| (about(mind.home, STROLL_WILD, rng), Why::Strolls));
    }
    if mind.flags & (FARM | FARM_LOOSE) != 0 {
        if let Some(fear) = near(SHY) {
            return Some((here + (here - fear), Why::StepsAway));
        }
        return strolls.then(|| (about(mind.home, STROLL_FARM, rng), Why::Strolls));
    }
    if mind.flags & GUARD != 0 {
        if strolls && !mind.dialogue {
            return Some((about(mind.home, STROLL_GUARD, rng), Why::Strolls));
        }
        let from_post = (here - mind.home).abs();
        return (from_post.x > POST || from_post.y > POST).then_some((mind.home, Why::GoesBack));
    }
    None
}

/// The hero's band, in which it strikes, `R + 45` (`0x4E0300` for a hero), and how far a hit
/// lands, 1.5 times the reach for a player's (`0x4EA1F0`; **hypothesis** that the base is `R`);
/// the reach is a creature's of the default size, the creatures' own.
const HERO_BAND: f32 = REACH + 45.0;
const HITS_WITHIN: f32 = 1.5 * REACH;
/// How much each chained strike raises the speed of the next, and the most (`0x4E4CF0`).
const CHAIN_STEP: f32 = 0.1;
const CHAIN_MOST: f32 = 1.7;

/// Whether the hero, its attack aimed at a point `distance` away on the ground, comes on toward
/// it (`0x6172C0`, the AI's state 4 for a hero): farther than its band, unless it strikes at
/// once, where it stands; else it stops, turns to it and strikes.
pub fn hero_comes_on(at_once: bool, distance: f32) -> bool {
    !at_once && distance > HERO_BAND
}

/// Whether the hero's strike's next hit is due (the attack action 3, `0x4E4CF0`), `done` of its
/// clip played and `struck` hits sent, by the clip's `fractions`: the first at its first
/// fraction, 0.5 for one of 0; a second at its second where there is one.
pub fn hit_due([first, second]: [f32; 2], struck: u8, done: f32) -> bool {
    let first = if first > 0.0 { first } else { 0.5 };
    match struck {
        0 => done >= first,
        1 => second > 0.0 && done >= second,
        _ => false,
    }
}

/// Whether a shot's arrow leaves, `done` of its clip played, `a` the clip's first fraction
/// (`MotionDesc +0xA0`, `0x4DD90E`): once what is left of the clip is at most `1 − a` of it, at
/// once for an `a` of 0, at the half for one of 1 or more.
pub fn looses(a: f32, done: f32) -> bool {
    let at = if a >= 1.0 { 0.5 } else { a.max(0.0) };
    done >= at
}

/// Whether the hero's hit lands on a target `distance` away on the ground: within 1.5 times the
/// reach.
pub fn hero_hits(distance: f32) -> bool {
    distance <= HITS_WITHIN
}

/// Whether the hero's attack ends with the strike that ends: a strike at once or in the air is
/// one, a click elsewhere ends it, and so does its target's death (not `alive`); else it strikes
/// again.
pub fn attack_ends(at_once: bool, ends: bool, alive: bool) -> bool {
    at_once || ends || !alive
}

/// The chain of the hero's strikes on a target (`+0x210`, `0x4E4CF0`): the factor of the speed
/// of its last strike, and the count of its strikes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chain {
    factor: f32,
    strikes: u32,
}

impl Default for Chain {
    fn default() -> Self {
        Self {
            factor: 1.0,
            strikes: 0,
        }
    }
}

impl Chain {
    /// The factor of the next strike: 1 for the first, else the last's raised by 0.1, at most
    /// 1.7.
    pub fn strike(&mut self) -> f32 {
        self.factor = if self.strikes == 0 {
            1.0
        } else {
            (self.factor + CHAIN_STEP).min(CHAIN_MOST)
        };
        self.strikes += 1;
        self.factor
    }

    /// Coming on again, the chain breaks.
    pub fn breaks(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEEDS: [f32; 2] = [70.0, 140.0];

    fn at(x: f32, y: f32) -> Spot {
        Spot::new(x, y)
    }

    #[test]
    fn a_creature_wounded_to_a_third_flees_once_near_a_hero_that_does_not_strike() {
        let goblin = Mind::new(Spot::default(), 0x1, (true, false), None, SPEEDS, (76, 2));
        let health = |current| (current, 72);
        assert!(flees(health(24), 150.0, &goblin, false, 0));
        // More than a third left, too far, the hero striking, or not this frame: it stays.
        assert!(!flees(health(25), 150.0, &goblin, false, 0));
        assert!(!flees(health(24), 200.0, &goblin, false, 0));
        assert!(!flees(health(24), 150.0, &goblin, true, 0));
        assert!(!flees(health(24), 150.0, &goblin, false, 3));
        // Once a fight, and never a creature of the classes 1 and 5.
        let fled = Mind {
            fled: true,
            ..goblin.clone()
        };
        assert!(!flees(health(10), 150.0, &fled, false, 0));
        let hero_like = Mind::new(Spot::default(), 0x1, (true, false), None, SPEEDS, (7, 1));
        assert!(!flees(health(10), 150.0, &hero_like, false, 0));
    }

    #[test]
    fn a_fleeing_creature_runs_on_and_a_coming_one_follows_the_hero() {
        let mut goblin = Mind::new(Spot::default(), 0x1, (true, false), None, SPEEDS, (76, 2));
        goblin.fighting = true;
        let mut rng = Rng::of(51, 39, 0);
        // 700 off, the hero far from where its walk heads: a flight runs on (#387), a walk that
        // comes on turns after the hero.
        let sight = |comes_on| Pursuit {
            here: at(0.0, 0.0),
            hero: at(150.0, 0.0),
            walk: Some(Walking {
                target: at(-700.0, 0.0),
                comes_on,
                runs: true,
            }),
            striking: false,
            health: Some((72, 72)),
            hero_strikes: true,
            listed: None,
        };
        assert_eq!(goblin.pursue(&sight(false), || true, &mut rng), None);
        assert!(matches!(
            goblin.pursue(&sight(true), || true, &mut rng),
            Some(Pursues::ComesOn { runs: true, .. })
        ));
    }

    /// A bandit with a bow (the type 640, class 3) fighting the hero, deciding.
    fn archer() -> Mind {
        let mut bandit = Mind::new(Spot::default(), 0x1, (true, false), None, SPEEDS, (640, 3));
        bandit.ranged = true;
        bandit.fighting = true;
        bandit
    }

    /// What an archer at the origin sees of the hero at `d` along x, standing or walking.
    fn at_range(d: f32, walk: Option<Walking>) -> Pursuit {
        Pursuit {
            here: Spot::default(),
            hero: at(d, 0.0),
            walk,
            striking: false,
            health: Some((72, 72)),
            hero_strikes: false,
            listed: None,
        }
    }

    #[test]
    fn an_archer_shoots_from_250_to_650_as_it_decides_and_in_its_band_as_it_moves() {
        let mut rng = Rng::of(48, 38, 1);
        for d in [250.0, 400.0, 650.0] {
            let mut bandit = archer();
            assert!(
                matches!(
                    bandit.pursue(&at_range(d, None), || true, &mut rng),
                    Some(Pursues::Shoots { .. })
                ),
                "{d}"
            );
        }
        // At 700 it moves to its band, where it shoots along a straight line, but not while its
        // shot plays.
        let mut bandit = archer();
        let shot = bandit.pursue(&at_range(700.0, None), || true, &mut rng);
        assert!(matches!(shot, Some(Pursues::Shoots { .. })), "{shot:?}");
        let playing = Pursuit {
            striking: true,
            ..at_range(400.0, None)
        };
        assert_eq!(bandit.pursue(&playing, || true, &mut rng), None);
        // Each shot is 0.1 faster than the last, up to 1.7 times.
        let chains: Vec<f32> = (0..10)
            .filter_map(
                |_| match bandit.pursue(&at_range(400.0, None), || true, &mut rng) {
                    Some(Pursues::Shoots { chain, .. }) => Some(chain),
                    _ => None,
                },
            )
            .collect();
        assert_eq!(chains.len(), 10);
        assert!((chains[0] - 1.1).abs() < 1e-5 && (chains[9] - 1.7).abs() < 1e-5);
    }

    #[test]
    fn an_archer_backs_off_ten_times_then_shoots_near() {
        let mut rng = Rng::of(48, 38, 2);
        let mut bandit = archer();
        let (mut runs, mut walks) = (0, 0);
        for _ in 0..10 {
            match bandit.pursue(&at_range(200.0, None), || true, &mut rng) {
                // As far again from the hero, away from it.
                Some(Pursues::BacksOff { to, runs: run }) => {
                    assert!((to - at(-200.0, 0.0)).length() < 1e-3, "{to:?}");
                    if run {
                        runs += 1;
                    } else {
                        walks += 1;
                    }
                }
                other => panic!("{other:?}"),
            }
        }
        assert!(runs > 0 && walks > 0, "{runs} {walks}");
        // While it walks, it walks on; the eleventh time it shoots where it stands.
        let walking = Some(Walking {
            target: at(-200.0, 0.0),
            comes_on: false,
            runs: false,
        });
        let mut fresh = archer();
        fresh.deciding = false;
        assert_eq!(
            fresh.pursue(&at_range(200.0, walking), || true, &mut rng),
            None
        );
        assert!(matches!(
            bandit.pursue(&at_range(200.0, None), || true, &mut rng),
            Some(Pursues::Shoots { .. })
        ));
        // A shot counts the backs anew.
        assert!(matches!(
            bandit.pursue(&at_range(200.0, None), || true, &mut rng),
            Some(Pursues::BacksOff { .. })
        ));
    }

    #[test]
    fn an_archer_comes_on_from_afar_goes_round_without_a_line_and_gives_up_beyond_2000() {
        let mut rng = Rng::of(48, 38, 3);
        let mut bandit = archer();
        assert_eq!(
            bandit.pursue(&at_range(900.0, None), || true, &mut rng),
            Some(Pursues::ComesOn {
                runs: false,
                within: 450.0
            })
        );
        // Coming on after the hero, it walks on; the hero gone elsewhere, it turns after it.
        let coming = |target| {
            Some(Walking {
                target,
                comes_on: true,
                runs: false,
            })
        };
        assert_eq!(
            bandit.pursue(&at_range(900.0, coming(at(900.0, 0.0))), || true, &mut rng),
            None
        );
        assert!(matches!(
            bandit.pursue(&at_range(900.0, coming(at(600.0, 0.0))), || true, &mut rng),
            Some(Pursues::ComesOn { .. })
        ));
        // In the band without a straight line, it goes round the hero, keeping its distance.
        for _ in 0..50 {
            let Some(Pursues::GoesRound { to, within, .. }) =
                bandit.pursue(&at_range(500.0, None), || false, &mut rng)
            else {
                panic!("goes round");
            };
            let hero = at(500.0, 0.0);
            let turned = (to - hero).y.atan2(-(to - hero).x).to_degrees().abs();
            assert!(
                ((to - hero).length() - 500.0).abs() < 0.01 && turned <= 30.01,
                "{to:?}"
            );
            assert_eq!(within, 25.0);
        }
        assert_eq!(
            bandit.pursue(&at_range(2100.0, None), || true, &mut rng),
            Some(Pursues::GivesUp)
        );
        assert_eq!(
            bandit.pursue(&at_range(400.0, None), || true, &mut rng),
            None
        );
    }

    /// A goblin warrior (76, class 8) fighting the hero in melee.
    fn goblin() -> Mind {
        let mut goblin = Mind::new(Spot::default(), 0x1, (true, false), None, SPEEDS, (76, 8));
        goblin.fighting = true;
        goblin
    }

    /// What a goblin at `here` sees of the hero at the origin, standing, the `index`-th of the
    /// hero's hostiles, whose first stands at `first`.
    fn listed(here: Spot, index: usize, first: Spot) -> Pursuit {
        Pursuit {
            here,
            hero: Spot::default(),
            walk: None,
            striking: false,
            health: Some((72, 72)),
            hero_strikes: false,
            listed: Some((index, first)),
        }
    }

    #[test]
    fn the_second_to_fourth_of_the_heros_hostiles_take_their_places_about_it() {
        let hero = at(1000.0, 1000.0);
        let first = at(880.0, 1000.0);
        let near = at(900.0, 1060.0);
        // Along the world's +x, −y and +y, 115 from the hero; the first and the fifth have none,
        // nor one 115 or more from the first.
        assert_eq!(place(near, hero, (1, first)), Some(at(1115.0, 1000.0)));
        assert_eq!(place(near, hero, (2, first)), Some(at(1000.0, 885.0)));
        assert_eq!(place(near, hero, (3, first)), Some(at(1000.0, 1115.0)));
        assert_eq!(place(near, hero, (0, first)), None);
        assert_eq!(place(near, hero, (4, first)), None);
        assert_eq!(place(at(880.0, 1115.0), hero, (1, first)), None);
        // The second, in the band, its place farther than 145 from it: it walks there for a
        // frame, stopping within 40, at a run in one case of two.
        let mut rng = Rng::of(51, 39, 4);
        let here = at(-100.0, 60.0);
        let first = at(-120.0, 0.0);
        let mut runs = 0;
        for _ in 0..200 {
            let mut second = goblin();
            match second.pursue(&listed(here, 1, first), || true, &mut rng) {
                Some(Pursues::TakesPlace {
                    to,
                    runs: run,
                    within,
                }) => {
                    assert_eq!((to, within), (at(115.0, 0.0), 40.0));
                    runs += usize::from(run);
                }
                other => panic!("{other:?}"),
            }
            // Then, walking there, it is steered by the hero alone: in the band it strikes.
            let walking = Pursuit {
                walk: Some(Walking {
                    target: at(115.0, 0.0),
                    comes_on: false,
                    runs: false,
                }),
                ..listed(here, 1, first)
            };
            assert!(matches!(
                second.pursue(&walking, || true, &mut rng),
                Some(Pursues::Strikes(Some(_)))
            ));
        }
        assert!((70..130).contains(&runs), "{runs}");
        // Beyond the band it comes on at its place's gait, nearer it steps back.
        let mut second = goblin();
        let far = at(-140.0, 60.0);
        let Some(Pursues::TakesPlace { runs, .. }) =
            second.pursue(&listed(far, 1, first), || true, &mut rng)
        else {
            panic!("takes its place");
        };
        assert_eq!(
            second.pursue(&listed(far, 1, first), || true, &mut rng),
            Some(Pursues::ComesOn {
                runs,
                within: 130.0
            })
        );
        let close = at(-60.0, 60.0);
        assert!(matches!(
            second.pursue(&listed(close, 1, first), || true, &mut rng),
            Some(Pursues::TakesPlace { .. })
        ));
        assert!(matches!(
            second.pursue(&listed(close, 1, first), || true, &mut rng),
            Some(Pursues::StepsBack(_))
        ));
        // One of class 5 walks; one whose place lies within 145 of it fights as the others.
        let mut undead = goblin();
        undead.class = 5;
        for _ in 0..50 {
            assert!(matches!(
                undead.pursue(&listed(here, 1, first), || true, &mut rng),
                Some(Pursues::TakesPlace { runs: false, .. })
            ));
            undead.placing = None;
        }
        let by_its_place = at(100.0, 60.0);
        let mut third = goblin();
        assert!(matches!(
            third.pursue(&listed(by_its_place, 1, at(30.0, 100.0)), || true, &mut rng),
            Some(Pursues::Strikes(Some(_)))
        ));
        // The first itself fights as the others.
        let mut nearest = goblin();
        assert!(matches!(
            nearest.pursue(&listed(first, 0, first), || true, &mut rng),
            Some(Pursues::Strikes(Some(_)))
        ));
    }

    #[test]
    fn a_creature_goes_for_the_nearest_closed_door_that_a_hero_turned() {
        let door = |door, x, layer, open, barred, marked| DoorSeen {
            door,
            at: at(x, 0.0),
            layer,
            open,
            barred,
            marked,
        };
        let doors = [
            door("open", 50.0, 0, true, false, true),
            door("unmarked", 60.0, 0, false, false, false),
            door("barred", 70.0, 0, false, true, true),
            door("upstairs", 80.0, 2, false, false, true),
            door("far", 900.0, 1, false, false, true),
            door("farther", -1000.0, 0, false, false, true),
        ];
        assert_eq!(door_in_the_way(Spot::default(), 0, doors), Some("far"));
        assert_eq!(door_in_the_way(Spot::default(), 2, doors), Some("upstairs"));
        assert_eq!(door_in_the_way(Spot::default(), 3, doors), None);
        assert_eq!(door_in_the_way::<&str>(Spot::default(), 0, []), None);
        // The ground and the first floor meet either way, no other two.
        assert!(meets(0, 1) && meets(1, 0) && meets(2, 2) && !meets(0, 2) && !meets(2, 1));
        // It opens a door within 300 on a layer that meets the door's.
        assert!(opens(at(0.0, 0.0), 0, at(300.0, 0.0), 1));
        assert!(!opens(at(0.0, 0.0), 0, at(250.0, 200.0), 0));
        assert!(!opens(at(0.0, 0.0), 2, at(10.0, 0.0), 1));
    }

    #[test]
    fn a_creature_without_a_line_to_the_hero_goes_for_a_door_or_gives_up() {
        let mut rng = Rng::of(50, 40, 7);
        let hero = at(500.0, 0.0);
        // A way to the hero, or to its point moved by up to 50: it comes on.
        let mut coming = goblin();
        let blocked = coming.blocked(hero, |_| true, || panic!("no door asked"), &mut rng);
        assert_eq!(blocked, Blocked::ComesOn);
        assert!(coming.fighting);
        let mut tries = Vec::new();
        assert_eq!(
            coming.blocked(
                hero,
                |to| {
                    tries.push(to);
                    tries.len() == 2
                },
                || false,
                &mut rng
            ),
            Blocked::ComesOn
        );
        let astray = tries[1] - hero;
        assert!(tries[0] == hero && astray.x.abs() <= 50.0 && astray.y.abs() <= 50.0);
        // No way: the door, leaving the fight, or else it gives up.
        let mut opener = goblin();
        assert_eq!(
            opener.blocked(hero, |_| false, || true, &mut rng),
            Blocked::Door
        );
        assert!(!opener.fighting && opener.to_door());
        let mut stuck = goblin();
        assert_eq!(
            stuck.blocked(hero, |_| false, || false, &mut rng),
            Blocked::GivesUp
        );
        assert!(!stuck.fighting && !stuck.to_door());
        // The beasts, the undead and the animals seek no door.
        for class in [2, 5, 6] {
            let mut beast = goblin();
            beast.class = class;
            let blocked = beast.blocked(hero, |_| false, || panic!("no door asked"), &mut rng);
            assert_eq!(blocked, Blocked::GivesUp);
        }
        // Going for its door it thinks nothing and hears no alarm, and at the door it looks for
        // the hero again at once.
        opener.group = Some(9);
        opener.hears_alarm(9);
        assert!(!opener.fighting);
        let senses = Senses {
            dt: 0.01,
            seen: true,
            here: Spot::default(),
            hero,
            hero_moves: false,
            takes_on: true,
            walking: false,
        };
        let thought = opener.think(&senses, || true, || true, &mut rng);
        assert_eq!(thought, Thought::default());
        opener.left_door();
        assert!(!opener.to_door() && opener.awake);
    }

    #[test]
    fn an_idle_creature_without_a_straight_line_goes_for_the_door_in_its_way() {
        let mut rng = Rng::of(50, 40, 8);
        let senses = Senses {
            dt: 0.01,
            seen: false,
            here: Spot::default(),
            hero: at(500.0, 0.0),
            hero_moves: false,
            takes_on: true,
            walking: false,
        };
        // A creature of a script, awake as it is made, by the hero: with a straight line it
        // attacks; without, it goes for a door that is found, and is busy with it.
        let made = || Mind::new(Spot::default(), 0x1, (false, true), None, SPEEDS, (76, 8));
        let mut sees = made();
        let thought = sees.think(&senses, || true, || panic!("no door asked"), &mut rng);
        assert!(thought.attacks && !thought.door);
        let mut blind = made();
        let thought = blind.think(&senses, || false, || true, &mut rng);
        assert!(thought.door && !thought.attacks && blind.to_door() && !blind.fighting);
        // Without a door it stays as it was; a beast seeks none.
        let mut doorless = made();
        let thought = doorless.think(&senses, || false, || false, &mut rng);
        assert!(!thought.door && !thought.attacks && !doorless.to_door());
        let mut beast = Mind::new(Spot::default(), 0x1, (false, true), None, SPEEDS, (75, 2));
        let thought = beast.think(&senses, || false, || panic!("no door asked"), &mut rng);
        assert!(!thought.door && !beast.to_door());
    }

    #[test]
    fn a_shots_arrow_leaves_at_its_clips_fraction() {
        assert!(looses(0.0, 0.0));
        assert!(!looses(0.4, 0.39) && looses(0.4, 0.4));
        assert!(!looses(1.2, 0.49) && looses(1.2, 0.5));
    }

    #[test]
    fn a_hostile_creature_of_a_script_wakes_as_it_is_made_and_looks_every_2_5_s() {
        let home = Spot::default();
        let script = Mind::new(home, 0x1, (false, true), None, SPEEDS, (0, 0));
        assert!(script.awake && script.period == PERIOD_HOSTILE);
        // A fill's hostile creature and a peaceful one sleep until woken, on a timer of 5 s.
        let fill = Mind::new(home, 0x1, (true, false), Some(7), SPEEDS, (0, 0));
        assert!(!fill.awake && fill.period == PERIOD);
        let villager = Mind::new(home, 0x400, (true, true), None, SPEEDS, (0, 0));
        assert!(!villager.awake && !villager.hostile());
    }

    #[test]
    fn a_peaceful_creature_of_an_enter_function_takes_a_behaviour_by_its_class() {
        let made = |flags, created, creature, class| {
            let made = (true, created);
            Mind::new(
                Spot::default(),
                flags,
                made,
                None,
                SPEEDS,
                (creature, class),
            )
            .flags
        };
        // A fill's animals by the creature maker's lists: the first rabbit wild, a pig farm, the
        // other rabbits without, so of class 6 the loose farm animals' at the first update.
        assert_eq!(made(of_type(516), false, 516, 6), WILD);
        assert_eq!(made(of_type(520), false, 520, 6), FARM);
        assert_eq!(made(of_type(592), false, 592, 6), FARM_LOOSE);
        // `CreateNPC`'s without a tag: class 6 the loose farm animals', the jugglers theirs, the
        // others the townsfolk's; a tag or hostility keeps its own.
        assert_eq!(made(0, true, 560, 6), FARM_LOOSE);
        assert_eq!(made(0, true, 78, 3), JUGGLER);
        assert_eq!(made(0, true, 640, 3), TOWNSFOLK);
        assert_eq!(made(0x4000, true, 640, 3), 0x4000);
        assert_eq!(made(0x1, false, 512, 6), 0x1);
        // An `Init` function's creature without a tag keeps none and stands.
        let init = Mind::new(Spot::default(), 0, (false, true), None, SPEEDS, (560, 6));
        assert_eq!(init.flags, 0);
    }

    #[test]
    fn a_wild_animal_flees_at_a_run_and_a_farm_animal_steps_away() {
        let mut rng = Rng::of(48, 39, 2);
        let home = at(1000.0, 1000.0);
        let rabbit = Mind::new(home, of_type(516), (true, false), None, SPEEDS, (516, 6));
        let pig = Mind::new(home, of_type(520), (true, false), None, SPEEDS, (520, 6));
        let hero = at(1100.0, 1000.0);
        // Within 300 of the hero the rabbit runs from it, by half to twice its distance along x
        // and across by up to half of none; beyond, it stands.
        for _ in 0..100 {
            let (to, why) = behaviour(&rabbit, home, Some(hero), false, &mut rng).unwrap();
            assert_eq!(why, Why::Flees);
            assert!(
                (800.0..=950.0).contains(&to.x) && (950.0..=1050.0).contains(&to.y),
                "{to:?}"
            );
        }
        let far = at(1400.0, 1000.0);
        assert_eq!(behaviour(&rabbit, home, Some(far), false, &mut rng), None);
        // Within 200 the pig walks to the point beyond itself; at 300 it stands.
        let (to, why) = behaviour(&pig, home, Some(hero), false, &mut rng).unwrap();
        assert!(
            why == Why::StepsAway && (to - at(900.0, 1000.0)).length() < 0.01,
            "{to:?}"
        );
        let near = at(1300.0, 1000.0);
        assert_eq!(behaviour(&pig, home, Some(near), false, &mut rng), None);
        // Farther than 380 from home the rabbit walks back within 50 of it.
        let strayed = home + at(0.0, 400.0);
        let (to, why) = behaviour(&rabbit, strayed, None, false, &mut rng).unwrap();
        let d = to - home;
        assert!(
            why == Why::GoesBack && d.x.abs() <= 50.0 && d.y.abs() <= 50.0,
            "{d:?}"
        );
        // A stroll stays within 100 of home for the rabbit, 150 for the pig.
        for _ in 0..200 {
            let (to, _) = behaviour(&rabbit, home, None, true, &mut rng).unwrap();
            let d = to - home;
            assert!(d.x.abs() <= 100.0 && d.y.abs() <= 100.0, "{d:?}");
            let (to, _) = behaviour(&pig, home, None, true, &mut rng).unwrap();
            let d = to - home;
            assert!(d.x.abs() <= 150.0 && d.y.abs() <= 150.0, "{d:?}");
        }
    }

    #[test]
    fn a_guard_strolls_about_its_post_and_goes_back_to_it() {
        let mut rng = Rng::of(50, 39, 4);
        let post = at(1000.0, 1000.0);
        let guard = Mind::new(post, 0x4000, (false, true), None, SPEEDS, (0, 3));
        // Within 90 of its post along each axis it stands; farther it walks back to it.
        let near = post + at(90.0, -90.0);
        assert_eq!(behaviour(&guard, near, None, false, &mut rng), None);
        let far = post + at(20.0, 95.0);
        assert_eq!(
            behaviour(&guard, far, None, false, &mut rng),
            Some((post, Why::GoesBack))
        );
        // By chance it strolls within 200 of its post, from wherever it stands; it fears no one.
        let hero = Some(far + at(1.0, 0.0));
        for _ in 0..100 {
            let (to, why) = behaviour(&guard, far, hero, true, &mut rng).unwrap();
            let d = to - post;
            assert!(
                why == Why::Strolls && d.x.abs() <= 200.0 && d.y.abs() <= 200.0,
                "{d:?}"
            );
        }
    }

    #[test]
    fn a_crowded_creature_parts_about_itself_but_near_the_worlds_diagonal() {
        let mut rng = Rng::of(50, 39, 5);
        let near = |to: Spot, at: Spot| (to - at).x.abs() <= 100.0 && (to - at).y.abs() <= 100.0;
        // At the start of the Seraphim, far from the diagonal: about itself, wherever home is.
        let here = at(173_688.0, 134_781.0);
        let home = here + at(250.0, 0.0);
        for _ in 0..50 {
            assert!(near(parting(home, here, &mut rng), here));
        }
        // Near the diagonal, its measure 30 to 400: about home.
        let here = at(10_000.0, 10_000.0);
        let home = at(10_100.0, 10_000.0);
        for _ in 0..50 {
            assert!(near(parting(home, here, &mut rng), home));
        }
        // On the diagonal at home, its measure 0: about itself.
        assert!(near(parting(here, here, &mut rng), here));
    }

    #[test]
    fn a_townsperson_rolls_home_an_animal_or_a_stop() {
        let mut rng = Rng::of(50, 39, 6);
        let home = at(1000.0, 1000.0);
        let farmer = Mind::new(home, TOWNSFOLK, (true, true), None, SPEEDS, (640, 3));
        let (pig, wife) = ("pig", "wife");
        let (mut homes, mut visits, mut stops) = (0, 0, 0);
        let (mut partners, mut walking, mut houses) = (0, 0, 0);
        for _ in 0..26_000 {
            match townsperson(&farmer, Some(wife), Some(pig), None, &mut rng) {
                Decision::GoesHome => homes += 1,
                Decision::Visits(animal) if animal == pig => visits += 1,
                Decision::Partner(partner, home) if partner == wife => {
                    partners += 1;
                    walking += usize::from(home);
                }
                Decision::House => houses += 1,
                Decision::Stop => stops += 1,
                other => panic!("{other:?}"),
            }
        }
        // Home in one roll of 26, the animal in one, a partner in two, one of them walking home.
        assert!((800..1_200).contains(&homes), "{homes}");
        assert!((800..1_200).contains(&visits), "{visits}");
        assert!((1_700..2_300).contains(&partners) && (800..1_200).contains(&walking));
        // A house in three rolls of 26.
        assert!((2_700..3_300).contains(&houses), "{houses}");
        assert_eq!(homes + visits + partners + houses + stops, 26_000);
        // Without a human or an animal those rolls walk home or stop.
        assert!((0..100).all(|_| !matches!(
            townsperson::<&str>(&farmer, None, None, None, &mut rng),
            Decision::Partner(..) | Decision::Visits(_)
        )));
        // Without an animal the roll of 1 stops; a child always rolls 20, a juggler 9.
        assert!((0..100).all(|_| roll(&farmer, &mut rng) != 9));
        let child = Mind::new(home, TOWNSFOLK, (true, true), None, SPEEDS, (702, 3));
        assert!((0..100).all(|_| roll(&child, &mut rng) == 20));
        let juggler = Mind::new(home, 0, (true, true), None, SPEEDS, (78, 3));
        assert_eq!(juggler.flags, JUGGLER);
        assert!((0..100).all(|_| roll(&juggler, &mut rng) == 9));
        assert!((0..100).all(|_| {
            townsperson(&juggler, Some(wife), Some(pig), None, &mut rng) == Decision::Juggle
        }));
    }

    #[test]
    fn a_townsperson_visits_the_second_nearest_trader_within_800() {
        let home = Spot::default();
        let smith = ("smith", at(300.0, 0.0));
        let wizard = ("wizard", at(700.0, 0.0));
        let merchant = ("merchant", at(900.0, 0.0));
        assert_eq!(
            trader_for(home, vec![wizard, smith, merchant]),
            Some("wizard")
        );
        assert_eq!(trader_for(home, vec![merchant, smith]), Some("smith"));
        assert_eq!(trader_for(home, vec![merchant]), Some("merchant"));
        assert_eq!(trader_for::<&str>(home, Vec::new()), None);
        // A citizen visits it in two rolls of 26, a farmer of the flag 8 in a third of those.
        let count = |flags| {
            let mind = Mind::new(home, flags, (true, true), None, SPEEDS, (652, 3));
            let mut rng = Rng::of(50, 39, 8);
            (0..26_000)
                .filter(|_| {
                    matches!(
                        townsperson(&mind, None, None, Some(smith.0), &mut rng),
                        Decision::Trade(_)
                    )
                })
                .count()
        };
        assert!(
            (1_700..2_300).contains(&count(CITIZEN)),
            "{}",
            count(CITIZEN)
        );
        assert!(
            (500..850).contains(&count(TOWNSFOLK)),
            "{}",
            count(TOWNSFOLK)
        );
    }

    #[test]
    fn a_child_plays_outside_and_gathers_about_the_hero_in_a_building() {
        let mut rng = Rng::of(50, 39, 9);
        let home = Spot::default();
        let kid = Mind::new(home, TOWNSFOLK, (true, true), None, SPEEDS, (702, 3));
        let hero = at(1000.0, 0.0);
        // Outside, it plays about itself near home, toward home from afar.
        for _ in 0..50 {
            let Child::Plays(to) = child(&kid, home, (hero, false), false, &mut rng) else {
                panic!("plays");
            };
            assert!(to.x.abs() <= 400.0 && to.y.abs() <= 400.0, "{to:?}");
            let far = at(2000.0, 0.0);
            let Child::Plays(to) = child(&kid, far, (hero, false), false, &mut rng) else {
                panic!("plays");
            };
            assert!((to - at(1200.0, 0.0)).length() < 0.01, "{to:?}");
        }
        // Inside, from afar it follows the hero (but in one case of 13 it plays).
        let follows = (0..1300)
            .filter(|_| child(&kid, home, (hero, false), true, &mut rng) == Child::FollowsHero)
            .count();
        assert!((1_150..1_250).contains(&follows), "{follows}");
        // Near, it mostly turns to the hero.
        let near = at(100.0, 0.0);
        let looks = (0..1300)
            .filter(|_| {
                matches!(
                    child(&kid, home, (near, false), true, &mut rng),
                    Child::LooksUp | Child::Speaks(_)
                )
            })
            .count();
        assert!((1_050..1_250).contains(&looks), "{looks}");
        // Before the Daemon it runs 700 away, within 400 more.
        let Child::Flees(to) = child(&kid, home, (near, true), true, &mut rng) else {
            panic!("flees");
        };
        assert!(
            (to.x + 700.0).abs() <= 400.0 && to.y.abs() <= 400.0,
            "{to:?}"
        );
    }

    #[test]
    fn a_townsperson_runs_700_from_its_threat() {
        let mut rng = Rng::of(50, 39, 10);
        let (here, goblin) = (Spot::default(), at(500.0, 0.0));
        for _ in 0..50 {
            let to = runs_from(here, goblin, &mut rng);
            assert!(
                (to.x + 700.0).abs() <= 100.0 && to.y.abs() <= 100.0,
                "{to:?}"
            );
        }
    }

    #[test]
    fn a_townsperson_follows_its_partner_and_meets_it() {
        let here = Spot::default();
        let partner = |d: f32| at(d, 0.0);
        assert_eq!(follow(here, partner(950.0), false), Why::GoesBack);
        assert_eq!(follow(here, partner(500.0), false), Why::Follows);
        assert_eq!(follow(here, partner(220.0), false), Why::GoesBack);
        assert_eq!(follow(here, partner(150.0), false), Why::Meets);
        assert_eq!(follow(here, partner(81.0), false), Why::Meets);
        assert_eq!(follow(here, partner(60.0), false), Why::StepsOff);
        // The hero is never left behind for its distance.
        assert_eq!(follow(here, partner(950.0), true), Why::Follows);
        assert_eq!(follow(here, partner(220.0), true), Why::GoesBack);
    }

    #[test]
    fn a_rate_a_second_comes_about_as_often_at_any_frame_rate() {
        let mut rng = Rng::of(0, 0, 3);
        for frames in [30.0_f32, 144.0] {
            let dt = 1.0 / frames;
            // 1,000 seconds' frames, about 297 strolls.
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a whole count of frames"
            )]
            let count = (1_000.0 * frames) as u32;
            let strolls = (0..count)
                .filter(|_| chance(&mut rng, STROLLS_A_SECOND, dt))
                .count();
            assert!((240..360).contains(&strolls), "{frames}: {strolls}");
        }
    }

    #[test]
    fn the_idle_action_steps_about_home_or_turns_about_the_hero() {
        let hero = at(1000.0, 1000.0);
        let here = at(1500.0, 1000.0);
        let mut rng = Rng::of(48, 39, 1);
        let script = Mind::new(at(1480.0, 990.0), 0x1, (false, true), None, SPEEDS, (0, 0));
        let fill = Mind::new(here, 0x1, (true, false), None, SPEEDS, (0, 0));
        let (mut steps, mut homes) = (0, 0);
        for _ in 0..2000 {
            if let Some(target) = idle_step(&script, here, hero, &mut rng) {
                if target == script.home {
                    homes += 1;
                } else {
                    let d = target - here;
                    assert!(d.x.abs() <= 150.0 && d.y.abs() <= 150.0, "{d:?}");
                    steps += 1;
                }
            }
            // An `Enter` function's creature keeps its distance from the hero.
            if let Some(target) = idle_step(&fill, here, hero, &mut rng) {
                let (before, after) = ((here - hero).length(), (target - hero).length());
                assert!((before - after).abs() < 0.01, "{before} {after}");
            }
        }
        // A step in about 22 % of the actions, home in a quarter of them.
        assert!((350..550).contains(&(steps + homes)), "{steps} {homes}");
        assert!(homes > 50 && homes < steps, "{steps} {homes}");
    }

    #[test]
    fn a_point_turns_about_the_vertical_as_the_worlds_do() {
        // A quarter turn takes the grid's x to its −y, and its y to its x.
        let x = Spot::new(1.0, 0.0).turned(90.0);
        let y = Spot::new(0.0, 1.0).turned(90.0);
        assert!((x - at(0.0, -1.0)).length() < 1e-6, "{x:?}");
        assert!((y - at(1.0, 0.0)).length() < 1e-6, "{y:?}");
        assert_eq!(Spot::default().unit(), Spot::default());
    }

    #[test]
    fn a_chain_of_strikes_grows_faster_by_a_tenth_up_to_1_7() {
        let mut chain = Chain::default();
        let factors: Vec<f32> = (0..10).map(|_| chain.strike()).collect();
        assert!((factors[0] - 1.0).abs() < 1e-6 && (factors[1] - 1.1).abs() < 1e-6);
        assert!((factors[7] - 1.7).abs() < 1e-5 && (factors[9] - 1.7).abs() < 1e-5);
        // Coming on again, it starts anew.
        chain.breaks();
        assert!((chain.strike() - 1.0).abs() < 1e-6);
    }
}
