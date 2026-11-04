//! The peace stream's alternation (docs/re/sound.md, "The peace stream's alternation"; #1177):
//! by day the region's music and the ambient loop of the place take the one stream in turns of
//! 180 s, crossfading over 7 s. The timer (`+0xB7B8`) counts by day and dawn alone; past 180 s it
//! switches the stream to the other phase and starts again, a crossfade that runs taking 20 s off
//! it first. The hero's death and the dusk and the night ask for the ambient loop now, setting the
//! timer to 200 s in the music's phase; a region's music that cannot open (none before the first
//! sector of music) keeps the ambient loop, once it plays. The scripts' control (`SetScriptSoundControle`, #1243)
//! refuses the switches while it holds, until the scripts give the stream back or 300 s pass.
//! `opensacred-game`'s `music` and `atmosphere` play what the phase holds.
//!
//! The fights' music and jingles ([`Fight`], docs/re/sound.md, "The fights' music and jingles";
//! #1948): once a second the sound manager weighs the foes about the hero, and plays a jingle
//! once or cuts in a fight's music, which the fight's end takes off again.

use crate::spawning::Dice;

/// How long a phase lasts, in seconds of the timer (`0x8961F0`).
pub const TURN: f64 = 180.0;
/// How much later a switch due while a crossfade runs is tried again (`0x896220`).
pub const WAITS: f64 = 20.0;
/// The timer that a request for the ambient loop sets, past [`TURN`] (`0x691310`).
pub const NOW: f64 = 200.0;
/// How long a crossfade lasts, in seconds (`0x69C4BB`).
pub const CROSSFADE: f32 = 7.0;
/// How long the scripts' control holds after their last `SetScriptSoundControle`, in seconds
/// (`+0xB7C8`, `0x6791C0`).
pub const CONTROL: f64 = 300.0;

/// The timer that a fight's end leaves where it stood past [`TURN`] (`0x697710`).
pub const AFTER_FIGHT: f64 = 120.0;
/// How long a fight's music is held off after the last one ends (`+0xAF70`).
pub const HELD_OFF: f64 = 10.0;
/// How often the fights are weighed, in seconds (`+0xD68`).
pub const WEIGHED: f64 = 1.0;
/// How long the foes must have been away for the jingles of a fight come back to them
/// (`+0xB7C0`).
pub const QUIET: f64 = 60.0;
/// How often the watch after a danger's jingle asks whether the fight is over (`+0xAF78`).
pub const WATCHED: f64 = 10.0;
/// The levels by which the foes' mean may stand above the hero's before the music of the
/// wrong level plays, by the difficulty, bronze to niobium and a sixth (`0x68D4C1`).
pub const WRONG_LEVEL: [f64; 6] = [10.0, 15.0, 20.0, 25.0, 30.0, 35.0];
/// `MUSIC_WRONGLEVEL`, the music of foes far above the hero.
pub const WRONG_LEVEL_MUSIC: u32 = 6569;
/// The regions' music that take a fight's music of their own: `MUSIC_DESERT01`,
/// `MUSIC_NUKNUK` and `MUSIC_KARIBIK` (`0x695E70`).
pub const DESERT: u32 = 0x196E;
pub const NUKNUK: u32 = 0x1A2F;
pub const KARIBIK: u32 = 0x1A30;
/// The fights' music by its index (`0x9DF15C`): `MUSIC_FIGHT01` to `08` but `07`, `09`, `10`,
/// `07`, `MUSIC_FIGHT_NUKNUK` and `MUSIC_FIGHT_KARIBIK`.
const FIGHT_TRACKS: [u32; 12] = [
    6530, 6531, 6532, 6533, 6534, 6535, 6564, 6537, 6565, 6536, 6709, 6708,
];

/// What the peace stream plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The region's music (`+0x698` 3).
    Music,
    /// The ambient loop of the place's nature (`+0x698` 2).
    Ambient,
}

/// The peace stream: its phase, none before the first region's event, its timer, and what is
/// left of the scripts' control, none when they do not hold it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stream {
    phase: Option<Phase>,
    timer: f64,
    control: Option<f64>,
}

impl Stream {
    /// The phase it plays, none before [`Self::begin`].
    pub fn phase(&self) -> Option<Phase> {
        self.phase
    }

    /// Its timer, in seconds.
    pub fn timer(&self) -> f64 {
        self.timer
    }

    /// Whether the scripts hold the stream (`+0x6B1`): no music but theirs changes it.
    pub fn controlled(&self) -> bool {
        self.control.is_some()
    }

    /// The scripts' `SetScriptSoundControle` (129, `0x6791C0`): `on` takes the stream for
    /// [`CONTROL`] from now, asking for the ambient loop in the music's phase as [`Self::now`]
    /// does; off gives it back, the caller crossfading to what the phase holds.
    pub fn control(&mut self, on: bool) {
        if on {
            self.control = Some(CONTROL);
            self.now();
        } else {
            self.control = None;
        }
    }

    /// Its first phase, as the first region's event rolls it (`0x6822E5`): the ambient loop for an
    /// odd `roll` (`rand() & 1`), else the music; the stream begun once.
    pub fn begin(&mut self, roll: u32) -> Phase {
        *self.phase.get_or_insert(if roll & 1 == 1 {
            Phase::Ambient
        } else {
            Phase::Music
        })
    }

    /// A frame of `seconds`: the scripts' control runs out at [`CONTROL`] whatever the hour
    /// (`0x69B85C`); the timer counts where the day `counts` (the day and the dawn, not the dusk
    /// nor the night); past [`TURN`], unless the place `holds` the cycle (the cemetery's and the
    /// indoor arena's music, a spot loop of their own), a crossfade that runs (`crossfading`)
    /// takes [`WAITS`] off it, and a timer still past it switches to the other phase, the music
    /// only where the place has one (`music`), the ambient loop only where [`ambient_loop`] gives
    /// one (`ambient`), nothing while the scripts hold the stream, and starts again
    /// (`0x69BD5E`). The phase it switched to, if it did.
    pub fn frame(
        &mut self,
        seconds: f64,
        (counts, holds): (bool, bool),
        (crossfading, music, ambient): (bool, bool, bool),
    ) -> Option<Phase> {
        if let Some(left) = self.control {
            self.control = Some(left - seconds).filter(|&left| left > 0.0);
        }
        if counts {
            self.timer += seconds;
        }
        let phase = self.phase?;
        if holds || self.timer <= TURN {
            return None;
        }
        if crossfading {
            self.timer -= WAITS;
            if self.timer <= TURN {
                return None;
            }
        }
        // A switch that the control refuses, tried again at each frame (`0x694616`).
        if self.controlled() {
            return None;
        }
        // A music or a loop that fails to open is tried again at each frame: where the chooser
        // gives no loop, the music plays on (`0x696A90` answering 0).
        let next = match phase {
            Phase::Music if !ambient => return None,
            Phase::Music => Phase::Ambient,
            Phase::Ambient if !music => return None,
            Phase::Ambient => Phase::Music,
        };
        self.phase = Some(next);
        self.timer = 0.0;
        Some(next)
    }

    /// A request for the music again (the fights' watch, `0x68BEA0`): in the ambient phase the
    /// timer to [`NOW`], so that the next frame switches to the music; in the music's phase
    /// nothing.
    pub fn back(&mut self) {
        if self.phase == Some(Phase::Ambient) {
            self.timer = NOW;
        }
    }

    /// The end of a fight's music (`0x697710`): a timer past [`TURN`] set back to
    /// [`AFTER_FIGHT`].
    pub fn after_fight(&mut self) {
        if self.timer > TURN {
            self.timer = AFTER_FIGHT;
        }
    }

    /// A request for the ambient loop now (`0x691310`; the hero's death `0x6853F7`, the dusk's
    /// and the night's events, a music that fails to open): in the music's phase the timer to
    /// [`NOW`], so that the next frame switches to the ambient loop; in the ambient phase nothing.
    pub fn now(&mut self) {
        if self.phase == Some(Phase::Music) {
            self.timer = NOW;
        }
    }
}

/// The fights' jingles, the `music` profile's groups 3, 4, 5 and 7.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Jingle {
    /// `JINGLE_DANGER`.
    Danger,
    /// `JINGLE_FIGHT_NORMAL`.
    Normal,
    /// `JINGLE_FIGHT_HARD`.
    Hard,
    /// `JINGLE_FIGHT_EASY`.
    Easy,
}

impl Jingle {
    /// Its group of the profiles `music` and `jingle_extension`.
    pub fn group(self) -> u8 {
        match self {
            Self::Danger => 3,
            Self::Normal => 4,
            Self::Hard => 5,
            Self::Easy => 7,
        }
    }
}

/// What a weighing of the fight asks of the music.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    /// A jingle once on the once-stream, of the profile 1 or 2 ([`Fight::update`]'s roll).
    Jingle(Jingle, u8),
    /// A fight's music of a kind, 0 to 2, cut in at once ([`fight_track`]).
    Fight(u8),
    /// [`WRONG_LEVEL_MUSIC`] by a crossfade, which counts as a fight's music.
    WrongLevel,
    /// The alternation's timer to [`NOW`] in the music's phase ([`Stream::now`]).
    Now,
    /// The alternation's timer to [`NOW`] in the ambient phase ([`Stream::back`]).
    Back,
    /// The fight's music ends (`0x697710`).
    End,
}

/// A foe as the fights' weighing sees it (`0x68BEA0`): its distance from the hero in the place's
/// pixels, its level, and its motions' shares of the last updates, per cent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Foe {
    pub distance: i32,
    pub level: u32,
    pub stands: u32,
    pub walks: u32,
    pub runs: u32,
    pub strikes: u32,
}

/// What the fights' weighing sees at an update: the hostile foes about the hero, the hero's
/// level, the most foes engaged over the last five updates, the stream's phase, whether the
/// once-stream plays, whether the region's music is the military siege's, the difficulty
/// (bronze 0 to niobium 4), and whether the hero stands out of the fight (`0x678F80`).
#[derive(Clone, Copy, Debug)]
pub struct Scene<'a> {
    pub foes: &'a [Foe],
    pub hero: u32,
    pub engaged: usize,
    pub phase: Phase,
    pub once: bool,
    pub siege: bool,
    pub difficulty: usize,
    pub calm: bool,
}

/// The foes within which the weighing counts them, in the place's pixels.
const WEIGHED_WITHIN: i32 = 700;
/// The strongest foe nearer than which a fight's music cuts in at once.
const CUTS_IN_WITHIN: i32 = 500;

/// The fights' music and jingles (`0x68BEA0`, docs/re/sound.md, "The fights' music and
/// jingles"): whether the hero has stood out of a fight since the last fade (`+0xAF24`), whether
/// a fight's music plays (`+0xAF4A`), the jingles' timers (the danger's and the easy fight's,
/// the normal fight's, the hard fight's), the fight's music held off, the watch after a danger's
/// jingle, the quiet without foes, and the clock of the weighing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fight {
    calm: bool,
    playing: bool,
    danger: f64,
    normal: f64,
    hard: f64,
    held: f64,
    watch: Option<f64>,
    quiet: f64,
    clock: f64,
}

impl Default for Fight {
    fn default() -> Self {
        Self {
            calm: true,
            playing: false,
            danger: 0.0,
            normal: 0.0,
            hard: 0.0,
            held: 0.0,
            watch: None,
            quiet: 0.0,
            clock: 0.0,
        }
    }
}

impl Fight {
    /// Whether a fight's music plays.
    pub fn playing(&self) -> bool {
        self.playing
    }

    /// The fade of the screen (the sound event (1, 2), `0x613210`): no longer calm.
    pub fn faded(&mut self) {
        self.calm = false;
    }

    /// Whether a fight's music may cut in now (`0x695E70`): not while one is held off or the
    /// once-stream plays; the caller refuses it also in the indoor arena and under the scripts'
    /// control.
    fn may_cut_in(&self, scene: &Scene) -> bool {
        self.held <= 0.0 && !scene.once
    }

    /// An update of `seconds` (`0x69BC16`, `0x68BEA0`): the timers fall, the jingles and the
    /// fight's music that the scene asks for, the watch, and out of the fight the end of its
    /// music. `refused` tells whether a fight's music would be refused where the hero stands
    /// (the indoor arena, the scripts' control); `dice` rolls `rand()`.
    pub fn update(
        &mut self,
        seconds: f64,
        scene: &Scene,
        refused: bool,
        dice: &mut impl Dice,
    ) -> Vec<Cue> {
        for timer in [
            &mut self.danger,
            &mut self.normal,
            &mut self.hard,
            &mut self.held,
        ] {
            if *timer > 0.0 {
                *timer -= seconds;
            }
        }
        let profile = 1 + u8::from(dice.below(2) == 1);
        let mut cues = Vec::new();
        let jingle = |cues: &mut Vec<Cue>, jingle: Jingle| {
            cues.push(Cue::Jingle(jingle, profile));
        };
        // The watch after a danger's jingle: out of the fight, the music asked back.
        if let Some(left) = self.watch {
            if left > 0.0 {
                self.watch = Some(left - seconds);
            } else if scene.calm {
                if scene.phase == Phase::Ambient {
                    cues.push(Cue::Back);
                }
                self.watch = None;
            } else {
                self.watch = Some(WATCHED);
            }
        }
        let ambient = scene.phase == Phase::Ambient;
        let busy = scene.once;
        let n = scene.engaged;
        // After a quiet minute, a jingle as foes engage again.
        if n == 0 {
            self.quiet += seconds;
        } else {
            if self.quiet > QUIET && !scene.siege && ambient && !self.playing && !busy {
                let (kind, timer) = match n {
                    1 => (Jingle::Easy, 30.0),
                    2 | 3 => (Jingle::Normal, 45.0),
                    _ => (Jingle::Hard, 60.0),
                };
                jingle(&mut cues, kind);
                match kind {
                    Jingle::Easy => self.danger = timer,
                    Jingle::Normal => self.normal = timer,
                    _ => self.hard = timer,
                }
            }
            self.quiet = 0.0;
        }
        self.clock += seconds;
        if !self.playing && self.clock > WEIGHED {
            self.clock = 0.0;
            self.weigh(scene, refused, dice, &mut cues, profile);
        }
        // Out of the fight: calm again, and the fight's music ends.
        if scene.calm {
            self.calm = true;
            if self.playing {
                self.playing = false;
                self.held = HELD_OFF;
                cues.push(Cue::End);
            }
        }
        cues
    }

    /// The danger's jingle in the music's phase: the alternation asked for the ambient loop, the
    /// danger's timer `timer` and the watch.
    fn warn(&mut self, cues: &mut Vec<Cue>, timer: f64, profile: u8) {
        cues.push(Cue::Now);
        cues.push(Cue::Jingle(Jingle::Danger, profile));
        self.danger = timer;
        self.watch = Some(WATCHED);
    }

    /// A fight's music of `kind` at once, where it may cut in.
    fn cut_in(&mut self, cues: &mut Vec<Cue>, kind: u8, scene: &Scene, refused: bool) {
        if !refused && self.may_cut_in(scene) {
            cues.push(Cue::Fight(kind));
            self.playing = true;
        }
    }

    /// The weighing once a second (`0x68BEA0` from `0x68CEA0`).
    fn weigh(
        &mut self,
        scene: &Scene,
        refused: bool,
        dice: &mut impl Dice,
        cues: &mut Vec<Cue>,
        profile: u8,
    ) {
        let near: Vec<&Foe> = scene
            .foes
            .iter()
            .filter(|foe| foe.distance < WEIGHED_WITHIN)
            .collect();
        let highest = near.iter().map(|foe| foe.level).max().unwrap_or(0);
        let measure = if near.is_empty() {
            0.0
        } else {
            let sum: f64 = near.iter().map(|foe| f64::from(foe.level)).sum();
            sum / f64::from(u32::try_from(near.len()).unwrap_or(u32::MAX)) - f64::from(scene.hero)
        };
        let n = scene.engaged;
        let ambient = scene.phase == Phase::Ambient;
        let busy = scene.once;
        let jingle = |cues: &mut Vec<Cue>, jingle: Jingle| cues.push(Cue::Jingle(jingle, profile));
        // The strongest foes, at least 4 levels above the hero.
        if scene.hero + 4 <= highest && n > 0 {
            for foe in near.iter().filter(|foe| foe.level == highest) {
                if foe.walks > 50 || foe.stands > 50 {
                    if ambient && self.danger <= 0.0 && self.calm && !busy {
                        jingle(cues, Jingle::Danger);
                        self.danger = 45.0;
                    }
                } else if foe.runs > 50 || foe.strikes > 30 {
                    if ambient && self.calm {
                        if self.hard <= 0.0 && !busy {
                            jingle(cues, Jingle::Hard);
                            self.hard = 45.0;
                        }
                    } else if !ambient && self.calm {
                        if !busy && !scene.siege {
                            self.warn(cues, 5.0, profile);
                        }
                    } else if !busy && foe.distance < CUTS_IN_WITHIN && scene.hero + 5 < highest {
                        self.held = 0.0;
                        self.cut_in(cues, 2, scene, refused);
                    }
                }
                if self.playing {
                    return;
                }
            }
        }
        if n == 0 {
            return;
        }
        // The normal fight's jingle, or the danger's in the music's phase.
        let normal = |fight: &mut Self, cues: &mut Vec<Cue>| {
            if ambient {
                if fight.normal <= 0.0 && !busy {
                    cues.push(Cue::Jingle(Jingle::Normal, profile));
                    fight.normal = 60.0;
                }
            } else if !busy && !scene.siege {
                fight.warn(cues, 4.0, profile);
            }
        };
        let level_test = |fight: &mut Self, cues: &mut Vec<Cue>| {
            fight.held = 0.0;
            let allowed = WRONG_LEVEL[scene.difficulty.min(5)];
            if measure <= allowed {
                fight.cut_in(cues, 2, scene, refused);
            } else if !refused {
                cues.push(Cue::WrongLevel);
                fight.playing = true;
            }
        };
        if n < 3 {
            if measure.abs() <= 1.0 {
                if ambient && self.danger <= 0.0 && !busy {
                    jingle(
                        cues,
                        if self.calm {
                            Jingle::Danger
                        } else {
                            Jingle::Easy
                        },
                    );
                    self.danger = 45.0;
                }
            } else if measure > 10.0 {
                level_test(self, cues);
            }
        } else if n <= 5 && measure <= 2.0 {
            if odd(dice) || n < 4 {
                normal(self, cues);
            } else {
                self.cut_in(cues, 0, scene, refused);
            }
        } else if n <= 5 && measure <= 10.0 {
            if odd(dice) {
                self.cut_in(cues, 1, scene, refused);
            } else {
                normal(self, cues);
            }
        } else if n <= 7 && measure <= 10.0 {
            if !odd(dice) {
                self.cut_in(cues, 1, scene, refused);
            } else if ambient {
                if self.hard <= 0.0 && !busy {
                    jingle(cues, Jingle::Hard);
                    self.hard = 60.0;
                }
            } else {
                self.held = 0.0;
                self.cut_in(cues, 2, scene, refused);
            }
        } else if n > 7 {
            self.cut_in(cues, 2, scene, refused);
        } else {
            level_test(self, cues);
        }
    }
}

/// Whether `rand() & 1` rolls odd.
fn odd(dice: &mut impl Dice) -> bool {
    dice.below(2) == 1
}

/// The track of a fight's music of `kind` in a sector of the music `region` (`0x695E70`):
/// `MUSIC_FIGHT_NUKNUK` and `_KARIBIK` in their regions, one of `MUSIC_FIGHT05` to `10` by
/// `roll6` (`rand() % 6`) in the desert's, else by the kind and the parity of `roll9`
/// (`rand() % 9`): the kind 0 `MUSIC_FIGHT01` odd and `05` even, 1 `04` and `06`, 2 `02` and
/// `03`.
pub fn fight_track(kind: u8, region: u32, roll6: u32, roll9: u32) -> u32 {
    let odd = roll9 % 2 == 1;
    let index = match region {
        NUKNUK => 10,
        KARIBIK => 11,
        DESERT => 4 + roll6 % 6,
        _ => match (kind, odd) {
            (0, true) => 0,
            (0, false) => 4,
            (1, true) => 3,
            (1, false) => 5,
            (2, true) => 1,
            _ => 2,
        },
    };
    FIGHT_TRACKS[usize::try_from(index).unwrap_or(0)]
}

/// The ambient loops (`ATMO_*`) and the musics that the ambient's choice takes as loops.
const ATMO_DESERT: u32 = 6500;
const ATMO_ICE: u32 = 6501;
const ATMO_VULCANO: u32 = 6502;
const ATMO_WOOD: u32 = 6503;
const ATMO_SIEGE_MILITARY_SUMMER: u32 = 6507;
const ATMO_DUNGEON: u32 = 6511;
const ATMO_DESERT_NIGHT: u32 = 6512;
const ATMO_ICE_NIGHT: u32 = 6513;
const ATMO_VULCANO_NIGHT: u32 = 6514;
const ATMO_WOOD_NIGHT: u32 = 6515;
const ATMO_DUNGEON_NIGHT: u32 = 6516;
const ATMO_SIEGE_MILITARY_WINTER: u32 = 6517;
const ATMO_KARIBIK: u32 = 6711;
const ATMO_PILZWALD: u32 = 6712;
const ATMO_HELL: u32 = 6713;
const MUSIC_PUB: u32 = 6518;
const MUSIC_SIEGE_MILITARY: u32 = 6551;
const MUSIC_SIEGE_UNDEAD: u32 = 6552;
/// The regions' musics whose own loops the day takes whatever the nature (`0x696C04`):
/// `MUSIC_PILZWALD`, `MUSIC_KARIBIK`, `MUSIC_HELL`, `MUSIC_WAYTOHELL` and `MUSIC_PIRATENPUB`.
const OWN_BY_DAY: [u32; 5] = [6701, 6704, 6707, 6706, 6715];
/// The nature `ARCTIC`, which takes the military siege's winter.
const ARCTIC: u8 = 0x20;

/// The ambient loop of a sector of the region's music `music`, its overrides applied (`+0x692`),
/// and of the sector's nature `nature` (`sectors.keyx +0x1D8`, `+0xA36`), by `night` (the dusk
/// and the night, `+0xA34` 1 or 2), as the sound manager chooses it (`0x696A90`; docs/re/sound.md,
/// "The atmosphere by the region"); none where it plays none, the loop that plays then staying.
///
/// - The undead siege's music plays on as its own loop; the pub's and the military siege's take
///   the nature's loop of the night at any hour, else their own (`MUSIC_PUB`,
///   `ATMO_VILLAGE_SIEGE_MILITARY_SUMMER`; the winter's, for `ARCTIC`, is never reached, as the
///   night's `ARCTIC` comes first).
/// - By night, the nature's loop of the night, else the music's.
/// - By day, the music's for the regions of [`OWN_BY_DAY`], else the nature's, else the music's.
pub fn ambient_loop(music: u32, nature: u8, night: bool) -> Option<u32> {
    let own = match music {
        MUSIC_SIEGE_UNDEAD => return Some(MUSIC_SIEGE_UNDEAD),
        MUSIC_PUB => MUSIC_PUB,
        MUSIC_SIEGE_MILITARY if nature == ARCTIC => ATMO_SIEGE_MILITARY_WINTER,
        MUSIC_SIEGE_MILITARY => ATMO_SIEGE_MILITARY_SUMMER,
        _ if night => return by_nature(nature, true).or_else(|| by_music(music, true)),
        _ if OWN_BY_DAY.contains(&music) => return by_music(music, false),
        _ => return by_nature(nature, false).or_else(|| by_music(music, false)),
    };
    Some(by_nature(nature, true).unwrap_or(own))
}

/// The ambient loop of a nature (`0x696C27` by day, `0x696BDA` by night): `DESERT` and `KIES`
/// the desert's, `ARCTIC` the ice's, `WALD`, `GRASS` and `SUMPF` the wood's, `LAVA` the
/// volcano's, `DUNGEON` the dungeon's by day and the wood's by night; none for the others.
fn by_nature(nature: u8, night: bool) -> Option<u32> {
    let (day, dark) = match nature {
        0x10 | 0xF0 => (ATMO_DESERT, ATMO_DESERT_NIGHT),
        0x20 => (ATMO_ICE, ATMO_ICE_NIGHT),
        0x30 | 0x40 | 0x50 => (ATMO_WOOD, ATMO_WOOD_NIGHT),
        0x60 => (ATMO_VULCANO, ATMO_VULCANO_NIGHT),
        0x80 => (ATMO_DUNGEON, ATMO_WOOD_NIGHT),
        _ => return None,
    };
    Some(if night { dark } else { day })
}

/// The ambient loop of a region's music (`0x696C6B` by day, `0x696D20` by night): the woods',
/// the villages', Mascarell's, the cemetery's, the outdoor arena's and the dryads' the wood's;
/// the dungeons' and the indoor arena's the dungeon's; the desert's, the lava's and the ice's
/// theirs; by day the mushroom forest's and the Caribbean's and the pirates' pub's their own and
/// the way to hell's and hell's the hell's; by night the mushroom forest's, Nuknuk's and the
/// Caribbean's the wood's and the way to hell's and hell's the hell's; none for the others (the
/// capital's, Nuknuk's by day, the pirates' pub's by night, the dungeon of Nuknuk's).
fn by_music(music: u32, night: bool) -> Option<u32> {
    let (day, dark) = match music {
        6504 | 6509 | 6544 | 6550 | 6560..=6562 | 6705 => (Some(ATMO_WOOD), Some(ATMO_WOOD_NIGHT)),
        6508 | 6520..=6523 => (Some(ATMO_DUNGEON), Some(ATMO_DUNGEON_NIGHT)),
        6510 => (Some(ATMO_DESERT), Some(ATMO_DESERT_NIGHT)),
        6540 => (Some(ATMO_VULCANO), Some(ATMO_VULCANO_NIGHT)),
        6563 => (Some(ATMO_ICE), Some(ATMO_ICE_NIGHT)),
        6701 => (Some(ATMO_PILZWALD), Some(ATMO_WOOD_NIGHT)),
        6703 => (None, Some(ATMO_WOOD_NIGHT)),
        6704 => (Some(ATMO_KARIBIK), Some(ATMO_WOOD_NIGHT)),
        6706 | 6707 => (Some(ATMO_HELL), Some(ATMO_HELL)),
        6715 => (Some(ATMO_KARIBIK), None),
        _ => (None, None),
    };
    if night { dark } else { day }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spawning::Script;

    #[test]
    fn the_ambient_loop_follows_the_regions_music_and_the_sectors_nature() {
        let (wood01, dungeon01, lava01, capital, pilzwald, karibik) =
            (6560, 6520, 6540, 6702, 6701, 6704);
        let (grass, kies, arctic, dungeon, water, road, floor) =
            (0x40, 0xF0, 0x20, 0x80, 0x90, 0xB0, 0xD0);
        // The start's 50,39 and the village's 52,39 on grass, by day and in the dusk (seen).
        assert_eq!(ambient_loop(wood01, grass, false), Some(ATMO_WOOD));
        assert_eq!(ambient_loop(6550, grass, true), Some(ATMO_WOOD_NIGHT));
        // A nature of no loop takes the music's: the floors of the woods, the lava's roads.
        assert_eq!(ambient_loop(wood01, floor, false), Some(ATMO_WOOD));
        assert_eq!(ambient_loop(lava01, road, false), Some(ATMO_VULCANO));
        assert_eq!(
            ambient_loop(dungeon01, floor, true),
            Some(ATMO_DUNGEON_NIGHT)
        );
        assert_eq!(
            ambient_loop(dungeon01, dungeon, true),
            Some(ATMO_WOOD_NIGHT)
        );
        assert_eq!(ambient_loop(6510, kies, false), Some(ATMO_DESERT));
        // The capital on its roads has none, by day and by night (seen: the music plays on).
        assert_eq!(ambient_loop(capital, road, false), None);
        assert_eq!(ambient_loop(capital, road, true), None);
        assert_eq!(ambient_loop(capital, grass, false), Some(ATMO_WOOD));
        // The regions' own loops by day whatever the nature (seen on the mushroom forest's
        // grass), the nature's by night.
        assert_eq!(ambient_loop(pilzwald, grass, false), Some(ATMO_PILZWALD));
        assert_eq!(ambient_loop(pilzwald, grass, true), Some(ATMO_WOOD_NIGHT));
        assert_eq!(ambient_loop(karibik, 0x10, false), Some(ATMO_KARIBIK));
        assert_eq!(ambient_loop(karibik, 0x10, true), Some(ATMO_DESERT_NIGHT));
        assert_eq!(ambient_loop(karibik, water, true), Some(ATMO_WOOD_NIGHT));
        assert_eq!(ambient_loop(6715, water, true), None);
        assert_eq!(ambient_loop(6707, floor, true), Some(ATMO_HELL));
        assert_eq!(ambient_loop(6703, grass, false), Some(ATMO_WOOD));
        assert_eq!(ambient_loop(6703, water, false), None);
        // The sieges: the undead's music on; the military's the night's nature at any hour,
        // else its own summer, never its winter.
        assert_eq!(
            ambient_loop(MUSIC_SIEGE_UNDEAD, grass, false),
            Some(MUSIC_SIEGE_UNDEAD)
        );
        assert_eq!(
            ambient_loop(MUSIC_SIEGE_MILITARY, grass, false),
            Some(ATMO_WOOD_NIGHT)
        );
        assert_eq!(
            ambient_loop(MUSIC_SIEGE_MILITARY, road, false),
            Some(ATMO_SIEGE_MILITARY_SUMMER)
        );
        assert_eq!(
            ambient_loop(MUSIC_SIEGE_MILITARY, arctic, false),
            Some(ATMO_ICE_NIGHT)
        );
        assert_eq!(ambient_loop(MUSIC_PUB, floor, false), Some(MUSIC_PUB));
        assert_eq!(ambient_loop(0, grass, false), Some(ATMO_WOOD));
        assert_eq!(ambient_loop(0, floor, false), None);
    }

    #[test]
    fn the_music_plays_on_where_the_place_has_no_ambient_loop() {
        let mut stream = Stream::default();
        stream.begin(0);
        assert_eq!(
            stream.frame(181.0, (true, false), (false, true, false)),
            None
        );
        assert_eq!(stream.frame(0.1, (true, false), (false, true, false)), None);
        assert_eq!(stream.phase(), Some(Phase::Music));
        assert_eq!(
            stream.frame(0.1, (true, false), (false, true, true)),
            Some(Phase::Ambient)
        );
    }

    /// Frames of `seconds` each, `count` of them, by day, nothing held nor fading, with music.
    fn run(stream: &mut Stream, seconds: f64, count: u32) -> Vec<Phase> {
        (0..count)
            .filter_map(|_| stream.frame(seconds, (true, false), (false, true, true)))
            .collect()
    }

    #[test]
    fn by_day_the_music_and_the_ambient_loop_take_turns_of_180_s() {
        let mut stream = Stream::default();
        // Nothing before the first region's event.
        assert_eq!(
            stream.frame(200.0, (true, false), (false, true, true)),
            None
        );
        assert_eq!(stream.begin(1), Phase::Ambient);
        // Begun once: a later roll changes nothing.
        assert_eq!(stream.begin(0), Phase::Ambient);
        let mut stream = Stream::default();
        stream.begin(1);
        // Seen: ATMO_WOOD at 20 s, the music 180 s later, the loop 180 s after it.
        assert!(run(&mut stream, 0.5, 360).is_empty());
        assert_eq!(run(&mut stream, 0.5, 1), [Phase::Music]);
        assert!(run(&mut stream, 0.5, 360).is_empty());
        assert_eq!(run(&mut stream, 0.5, 1), [Phase::Ambient]);
    }

    #[test]
    fn the_dusk_and_the_night_hold_the_timer_and_ask_for_the_ambient_loop() {
        let mut stream = Stream::default();
        stream.begin(0);
        assert_eq!(stream.phase(), Some(Phase::Music));
        // The dusk asks for the loop: the next frame switches, whatever the timer.
        stream.now();
        assert_eq!(
            stream.frame(0.1, (false, false), (false, true, true)),
            Some(Phase::Ambient)
        );
        // The night counts nothing: no music all night.
        for _ in 0..10_000 {
            assert_eq!(stream.frame(0.1, (false, false), (false, true, true)), None);
        }
        assert!(stream.timer() < 1.0);
        // A request in the ambient phase changes nothing.
        stream.now();
        assert_eq!(stream.frame(0.1, (false, false), (false, true, true)), None);
        // The dawn counts again: the music 180 s into it.
        assert!(run(&mut stream, 0.5, 360).is_empty());
        assert_eq!(run(&mut stream, 0.5, 1), [Phase::Music]);
    }

    #[test]
    fn the_scripts_control_holds_the_stream_until_given_back_or_300_s_pass() {
        let mut stream = Stream::default();
        stream.begin(0);
        // Taken in the music's phase: the loop asked for, but refused while held.
        stream.control(true);
        assert!(stream.controlled());
        assert!((stream.timer() - NOW).abs() < 1e-9);
        assert_eq!(stream.frame(0.5, (true, false), (false, true, true)), None);
        assert_eq!(stream.phase(), Some(Phase::Music));
        // Given back: the switch goes through at the next frame.
        stream.control(false);
        assert_eq!(
            stream.frame(0.5, (true, false), (false, true, true)),
            Some(Phase::Ambient)
        );
        // The control runs out 300 s after it was taken, the switch due then going through.
        stream.control(true);
        assert_eq!(stream.phase(), Some(Phase::Ambient));
        assert!(run(&mut stream, 0.5, 599).is_empty());
        assert!(stream.controlled());
        assert_eq!(run(&mut stream, 0.5, 1), [Phase::Music]);
        assert!(!stream.controlled());
    }

    /// A scene of `foes` about a hero of level 1, `engaged` of them engaged, in `phase`.
    fn scene(foes: &[Foe], engaged: usize, phase: Phase) -> Scene<'_> {
        Scene {
            foes,
            hero: 1,
            engaged,
            phase,
            once: false,
            siege: false,
            difficulty: 0,
            calm: false,
        }
    }

    /// A foe of `level` 100 pixels off, striking.
    fn striking(level: u32) -> Foe {
        Foe {
            distance: 100,
            level,
            strikes: 60,
            ..Foe::default()
        }
    }

    /// The cues of a second's update.
    fn second(fight: &mut Fight, scene: &Scene, dice: &mut Script) -> Vec<Cue> {
        fight.update(1.1, scene, false, dice)
    }

    #[test]
    fn a_fights_music_takes_its_track_by_the_kind_and_the_region() {
        assert_eq!(fight_track(0, 0x1980, 0, 1), 6530);
        assert_eq!(fight_track(0, 0x1980, 0, 4), 6534);
        assert_eq!(fight_track(1, 0x1980, 0, 7), 6533);
        assert_eq!(fight_track(2, 0x1980, 0, 8), 6532);
        assert_eq!(fight_track(2, NUKNUK, 3, 3), 6709);
        assert_eq!(fight_track(0, KARIBIK, 3, 3), 6708);
        // The desert's six: 05, 06, 09, 08, 10, 07.
        let desert: Vec<u32> = (0..6).map(|roll| fight_track(1, DESERT, roll, 0)).collect();
        assert_eq!(desert, [6534, 6535, 6564, 6537, 6565, 6536]);
    }

    #[test]
    fn many_foes_cut_in_a_fights_music_which_their_end_takes_off() {
        let foes = [striking(1); 9];
        let mut dice = Script::new(Vec::new());
        let mut fight = Fight::default();
        // Nine foes about a hero of their level: the kind 2 at once.
        let cues = second(&mut fight, &scene(&foes, 9, Phase::Music), &mut dice);
        assert_eq!(cues, [Cue::Fight(2)]);
        assert!(fight.playing());
        // While it plays nothing more is weighed.
        assert!(second(&mut fight, &scene(&foes, 9, Phase::Music), &mut dice).is_empty());
        // Out of the fight it ends, and the next one waits 10 s.
        let mut calm = scene(&[], 0, Phase::Music);
        calm.calm = true;
        assert_eq!(second(&mut fight, &calm, &mut dice), [Cue::End]);
        assert!(!fight.playing());
        let cues = second(&mut fight, &scene(&foes, 9, Phase::Music), &mut dice);
        assert!(cues.is_empty());
        for _ in 0..9 {
            second(&mut fight, &scene(&foes, 9, Phase::Music), &mut dice);
        }
        assert!(fight.playing());
    }

    #[test]
    fn foes_far_above_the_hero_play_the_wrong_levels_music_by_the_difficulty() {
        let foes = [striking(20); 2];
        let mut dice = Script::new(Vec::new());
        // Bronze: 19 levels above, the wrong level's music.
        let mut fight = Fight::default();
        let cues = second(&mut fight, &scene(&foes, 2, Phase::Ambient), &mut dice);
        assert!(cues.contains(&Cue::WrongLevel));
        // Gold allows 20: the kind 2 instead.
        let mut fight = Fight::default();
        let near = [striking(15); 2];
        let mut gold = scene(&near, 2, Phase::Ambient);
        gold.difficulty = 2;
        let cues = second(&mut fight, &gold, &mut dice);
        assert!(cues.contains(&Cue::Fight(2)), "{cues:?}");
    }

    #[test]
    fn a_few_foes_of_the_heros_level_sound_a_jingle_in_the_ambient_phase() {
        let foes = [striking(1)];
        let mut dice = Script::new(Vec::new());
        let mut fight = Fight::default();
        // Calm since the start: the danger's jingle, then 45 s of silence.
        let cues = second(&mut fight, &scene(&foes, 1, Phase::Ambient), &mut dice);
        assert_eq!(cues, [Cue::Jingle(Jingle::Danger, 1)]);
        for _ in 0..30 {
            assert!(second(&mut fight, &scene(&foes, 1, Phase::Ambient), &mut dice).is_empty());
        }
        // After a fade, the easy fight's.
        let mut fight = Fight::default();
        fight.faded();
        let cues = second(&mut fight, &scene(&foes, 1, Phase::Ambient), &mut dice);
        assert_eq!(cues, [Cue::Jingle(Jingle::Easy, 1)]);
        // In the music's phase, nothing.
        let mut fight = Fight::default();
        assert!(second(&mut fight, &scene(&foes, 1, Phase::Music), &mut dice).is_empty());
    }

    #[test]
    fn the_danger_in_the_music_s_phase_gives_way_to_the_loop_and_asks_the_music_back() {
        // Three foes of the hero's level, an odd roll: the normal fight's jingle, in the music's
        // phase the danger's with the watch.
        let foes = [striking(1); 3];
        let mut dice = Script::new(vec![1]);
        let mut fight = Fight::default();
        let cues = second(&mut fight, &scene(&foes, 3, Phase::Music), &mut dice);
        assert_eq!(cues, [Cue::Now, Cue::Jingle(Jingle::Danger, 2)]);
        // Out of the fight 10 s later, in the ambient phase: the music asked back.
        let mut calm = scene(&[], 0, Phase::Ambient);
        calm.calm = true;
        let mut back = Vec::new();
        for _ in 0..12 {
            back.extend(fight.update(1.0, &calm, false, &mut dice));
        }
        assert_eq!(back, [Cue::Back]);
    }

    #[test]
    fn foes_back_after_a_quiet_minute_sound_their_jingle() {
        let mut dice = Script::new(Vec::new());
        let mut fight = Fight::default();
        let quiet = scene(&[], 0, Phase::Ambient);
        fight.update(61.0, &quiet, false, &mut dice);
        // Four foes far off (beyond the weighing): the hard fight's jingle, by their count alone.
        let far = [Foe {
            distance: 900,
            level: 1,
            ..Foe::default()
        }; 4];
        let cues = fight.update(0.1, &scene(&far, 4, Phase::Ambient), false, &mut dice);
        assert_eq!(cues, [Cue::Jingle(Jingle::Hard, 1)]);
    }

    #[test]
    fn a_switch_waits_for_a_crossfade_and_a_place_without_music() {
        let mut stream = Stream::default();
        stream.begin(0);
        stream.now();
        // A crossfade running: 20 s off, the switch where the timer is still past 180 s.
        assert_eq!(stream.frame(0.0, (true, false), (true, true, true)), None);
        assert!((stream.timer() - (NOW - WAITS)).abs() < 1e-9);
        assert_eq!(stream.frame(0.0, (true, false), (false, true, true)), None);
        assert_eq!(
            stream.frame(0.5, (true, false), (false, true, true)),
            Some(Phase::Ambient)
        );
        let mut stream = Stream::default();
        stream.begin(0);
        stream.now();
        assert_eq!(
            stream.frame(0.5, (true, false), (true, true, true)),
            Some(Phase::Ambient)
        );
        // No music where the hero stands: the loop stays, tried again at each frame.
        let mut stream = Stream::default();
        stream.begin(1);
        assert_eq!(
            stream.frame(181.0, (true, false), (false, false, true)),
            None
        );
        assert_eq!(stream.frame(0.1, (true, false), (false, false, true)), None);
        assert_eq!(
            stream.frame(0.1, (true, false), (false, true, true)),
            Some(Phase::Music)
        );
        // The cemetery holds the cycle.
        let mut stream = Stream::default();
        stream.begin(1);
        assert_eq!(stream.frame(500.0, (true, true), (false, true, true)), None);
        assert_eq!(stream.phase(), Some(Phase::Ambient));
    }
}
