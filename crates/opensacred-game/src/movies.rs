//! The movies (#1945; docs/re/movies.md): the converted WebM's AV1 frames and Opus sound
//! (`video/<name>.webm`, `opensacred_data::video`) played over everything, filling the original's
//! 4:3 screen of the interface (`bar::SCREEN`) with the rest of a wider window black, until the
//! movie ends or Escape, Space (the controls' `skip` of `[movie]`) or either mouse button held
//! stops it (`0x6A0EA0`).
//! As the original's player (`0x6A16E0`), it waits half a second before a movie, and a quarter of
//! a second after it and then while Escape or Space is still held; the keys and the clicks of the
//! whole while are thrown away, and a movie whose file is not there is skipped.
//! A game stands under a movie, its time and its sound, as under the pause without its menu
//! (`pause::Still`; **hypothesis**: the player's event `(0xC, 8, 1)` stops the music); in the
//! menus their music and sounds pause.
//! The movies play where the original's do (#1946): the start's logos before the main menu, the
//! scripts' `InfoPlayer`s and `GameEnd`, unless the settings' `[movies] show` is off, and the
//! Extras' and the console's `movie` whatever it says; an extro played is kept in the settings,
//! for the Extras to offer it.
//!
//! A thread of its own reads the file through Symphonia's Matroska demuxer: the sound first,
//! decoded whole by libopus as the game's other sound is (`opus`), then the frames one by one by
//! rav1d, each turned from its YUV into RGBA and handed over a short queue, from which each frame of
//! the game shows the newest whose time has come on the movie's clock.

use std::collections::VecDeque;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TryRecvError};
use std::sync::{Mutex, PoisonError};
use std::thread;

use anyhow::{Context, Result, bail, ensure};
use bevy::asset::RenderAssetUsages;
use bevy::input::InputSystems;
use bevy::prelude::{
    App, Assets, BackgroundColor, ButtonInput, ChildOf, Color, Commands, Entity, GlobalZIndex,
    Handle, Image, ImageNode, IntoScheduleConfigs, KeyCode, Message, MessageReader, MessageWriter,
    MouseButton, NextState, Node, OnEnter, Plugin, PositionType, PreUpdate, Real, Res, ResMut,
    Resource, State, Time, UVec2, UiRect, Update, Val, default, info, resource_exists, warn,
};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_kira_audio::prelude::{Audio, AudioApp, AudioChannel, AudioControl, AudioSource};
use kira::sound::static_sound::StaticSoundData;
use rav1d::pixel::{MatrixCoefficients, YUVRange};
use rav1d::{Decoder, PixelLayout, PlanarImageComponent, Rav1dError};
use symphonia::core::codecs::video::well_known::CODEC_ID_AV1;
use symphonia::core::codecs::video::well_known::extra_data::VIDEO_EXTRA_DATA_ID_AV1_DECODER_CONFIG;
use symphonia::core::formats::TrackType;
use symphonia::core::formats::probe::Hint;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::units::TimeBase;

use crate::controls::{Action, Keys};
use crate::frame::{Hold, HoldsControl};
use crate::game::AppState;
use crate::pause::{Play, Still};
use crate::settings::{Settings, SettingsFile};
use crate::{bar, files, music, opus, scripts, talk};

/// The directory of the movies in the assets.
const DIR: &str = "video";
/// How long the player waits before a movie and after it, in seconds (`0x6A16E0`).
const BEFORE: f64 = 0.5;
const AFTER: f64 = 0.25;
/// How many decoded frames wait for their time, ahead of the one shown.
const AHEAD: usize = 4;
/// The decoder's threads: the movies are 640 × 480 at 25 frames a second, and the game stands.
const THREADS: u32 = 2;
/// The movies' layer of the interface: over the windows, the cursor's item (`held::OVER_ALL`) and
/// the art slots' choice, under the tools (`journal`, `overlay`, `help`, `console`).
const LAYER: i32 = i32::MAX - 3;
/// The buttons that stop a movie while held, beside the keys of `Action::SkipMovie`
/// (`0x6A0EA0`).
const STOP_BUTTONS: [MouseButton; 2] = [MouseButton::Left, MouseButton::Right];

/// Movies asked for by their names (`intro`, `act1`, …), to play in turn, those not there
/// skipped: played unless one plays already.
#[derive(Message, Debug, Clone, PartialEq)]
pub struct Show {
    pub names: Vec<String>,
    /// Whether their screen is black from the request to their end: the start's logos play
    /// before anything shows (`initApp`), the others over what they stand, which shows through
    /// the wait before them.
    pub black: bool,
}

impl Show {
    /// The movies `names`, over what they stand.
    pub fn of(names: Vec<String>) -> Self {
        Self {
            names,
            black: false,
        }
    }

    /// The movie `name` alone.
    pub fn one(name: &str) -> Self {
        Self::of(vec![name.to_owned()])
    }
}

/// The movies' sound channel, apart from the game's, which stands under a movie.
#[derive(Resource)]
pub struct Channel;

/// The asset path of the movie `name`.
fn path(name: &str) -> PathBuf {
    files::path(Path::new(DIR).join(opensacred_data::video::file(
        name,
        opensacred_data::video::EXTENSION,
    )))
}

/// Whether the movie `name` is in the assets.
pub fn exists(name: &str) -> bool {
    files::exists(path(name))
}

/// What a movie's thread reads before its first frame: its sound, its frames' size and its
/// length.
struct Opened {
    sound: Option<StaticSoundData>,
    size: UVec2,
    /// The end of its last frame or of its sound, the later, in seconds.
    length: f64,
}

/// A decoded frame: its time from the movie's start, in seconds, and its RGBA pixels.
struct Picture {
    at: f64,
    rgba: Vec<u8>,
}

/// A movie's frames, as the file holds them.
struct Video {
    /// The track's configuration's sequence header (its `av1C` after its first four bytes), which
    /// leads the first frame.
    header: Vec<u8>,
    /// Each frame's time, in seconds, and its data.
    packets: Vec<(f64, Box<[u8]>)>,
}

/// A packet's time on its track's `base`, in seconds.
fn seconds(base: TimeBase, ts: symphonia::core::units::Timestamp) -> f64 {
    base.calc_time_saturating(ts).as_secs_f64()
}

/// Reads a movie file's `bytes`: its sound decoded, its frames' packets kept.
fn demux(bytes: Vec<u8>) -> Result<(Opened, Video)> {
    let source = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(opensacred_data::video::EXTENSION);
    let mut format = symphonia::default::get_probe()
        .probe(&hint, source, Default::default(), Default::default())
        .context("reading the container")?;
    let track = format
        .default_track(TrackType::Video)
        .context("no video track")?;
    let (video_id, video_base) = (track.id, track.time_base.context("no time base")?);
    let params = track
        .codec_params
        .as_ref()
        .and_then(|params| params.video())
        .context("no video parameters")?;
    ensure!(params.codec == CODEC_ID_AV1, "the video is not AV1");
    let size = UVec2::new(
        params.width.map(u32::from).context("no width")?,
        params.height.map(u32::from).context("no height")?,
    );
    let header = params
        .extra_data
        .iter()
        .find(|extra| extra.id == VIDEO_EXTRA_DATA_ID_AV1_DECODER_CONFIG)
        .and_then(|extra| extra.data.get(4..))
        .map(<[u8]>::to_vec)
        .unwrap_or_default();
    let audio = format.default_track(TrackType::Audio).and_then(|track| {
        let params = track.codec_params.as_ref()?.audio()?;
        Some((track.id, params.clone()))
    });
    let (audio_id, mut sound) = match audio {
        Some((id, params)) => (Some(id), Some(opus::Sound::new(&params)?)),
        None => (None, None),
    };
    let mut packets = Vec::new();
    let mut length = 0.0f64;
    while let Some(packet) = format.next_packet().context("reading a packet")? {
        if packet.track_id == video_id {
            let at = seconds(video_base, packet.pts);
            let end = packet
                .pts
                .checked_add(packet.dur)
                .map_or(at, |end| seconds(video_base, end));
            length = length.max(end);
            packets.push((at, packet.data));
        } else if Some(packet.track_id) == audio_id
            && let Some(sound) = &mut sound
        {
            sound.decode(&packet).context("decoding the sound")?;
        }
    }
    let sound = sound.map(opus::Sound::data);
    if let Some(sound) = &sound {
        let frames = u32::try_from(sound.frames.len()).context("a sound too long")?;
        length = length.max(f64::from(frames) / f64::from(sound.sample_rate));
    }
    Ok((
        Opened {
            sound,
            size,
            length,
        },
        Video { header, packets },
    ))
}

/// What [`take`] found.
enum Took {
    Frame,
    Nothing,
    /// The player has gone: the movie stopped.
    Gone,
}

/// Takes a decoded frame from `decoder` into `pictures`, its time from `times` by its index.
fn take(decoder: &mut Decoder, times: &[f64], pictures: &SyncSender<Picture>) -> Result<Took> {
    let picture = match decoder.get_picture() {
        Ok(picture) => picture,
        Err(Rav1dError::TryAgain) => return Ok(Took::Nothing),
        Err(e) => return Err(e).context("decoding a frame"),
    };
    let at = picture
        .timestamp()
        .and_then(|index| usize::try_from(index).ok())
        .and_then(|index| times.get(index))
        .copied()
        .context("a frame of no packet")?;
    let rgba = rgba(&picture)?;
    Ok(match pictures.send(Picture { at, rgba }) {
        Ok(()) => Took::Frame,
        Err(_) => Took::Gone,
    })
}

impl Video {
    /// Decodes the frames in order into `pictures`, until the last or until the movie stops.
    fn decode(self, pictures: &SyncSender<Picture>) -> Result<()> {
        let mut settings = rav1d::Settings::new();
        settings.set_n_threads(THREADS);
        let mut decoder = Decoder::with_settings(&settings).context("making the decoder")?;
        let (times, data): (Vec<f64>, Vec<Box<[u8]>>) = self.packets.into_iter().unzip();
        let mut header = Some(self.header).filter(|header| !header.is_empty());
        for (index, data) in data.into_iter().enumerate() {
            // The sequence header leads the first frame, as the decoder needs it first.
            let data = match header.take() {
                Some(mut header) => {
                    header.extend_from_slice(&data);
                    header.into_boxed_slice()
                }
                None => data,
            };
            let mut sent = decoder.send_data(data, None, Some(i64::try_from(index)?), None);
            loop {
                let pending = match sent {
                    Ok(()) => false,
                    Err(Rav1dError::TryAgain) => true,
                    Err(e) => return Err(e).context("sending a frame"),
                };
                if let Took::Gone = take(&mut decoder, &times, pictures)? {
                    return Ok(());
                }
                if !pending {
                    break;
                }
                sent = decoder.send_pending_data();
            }
        }
        // The frames still in the decoder.
        while let Took::Frame = take(&mut decoder, &times, pictures)? {}
        Ok(())
    }
}

/// The thread of the movie at `path`: its sound and size into `opened`, then its frames into
/// `pictures`.
fn read(path: &Path, opened: &Sender<Result<Opened, String>>, pictures: &SyncSender<Picture>) {
    let demuxed = files::read(path)
        .with_context(|| format!("reading {}", path.display()))
        .and_then(demux);
    match demuxed {
        Err(e) => {
            // The player has gone if it is not there to hear.
            opened.send(Err(format!("{e:#}"))).ok();
        }
        Ok((movie, video)) => {
            if opened.send(Ok(movie)).is_ok()
                && let Err(e) = video.decode(pictures)
            {
                warn!("the movie {}: {e:#}", path.display());
            }
        }
    }
}

/// The conversion of a frame's YUV into RGB, in 256ths: the luma's black and scale, and the
/// chroma's weights in the red, the green and the blue.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Matrix {
    black: i32,
    luma: i32,
    rv: i32,
    gu: i32,
    gv: i32,
    bu: i32,
}

impl Matrix {
    /// BT.709's for a frame that tells it, BT.601's for the rest, as a movie of standard
    /// definition is, in the limited range or the full.
    fn of(matrix: MatrixCoefficients, range: YUVRange) -> Self {
        let bt709 = matrix == MatrixCoefficients::BT709;
        match (bt709, range) {
            (false, YUVRange::Limited) => Self {
                black: 16,
                luma: 298,
                rv: 409,
                gu: 100,
                gv: 208,
                bu: 516,
            },
            (true, YUVRange::Limited) => Self {
                black: 16,
                luma: 298,
                rv: 459,
                gu: 55,
                gv: 136,
                bu: 541,
            },
            (false, YUVRange::Full) => Self {
                black: 0,
                luma: 256,
                rv: 359,
                gu: 88,
                gv: 183,
                bu: 454,
            },
            (true, YUVRange::Full) => Self {
                black: 0,
                luma: 256,
                rv: 403,
                gu: 48,
                gv: 120,
                bu: 475,
            },
        }
    }

    /// The RGB of a pixel's `y`, `u` and `v`.
    fn rgb(self, y: u8, u: u8, v: u8) -> [u8; 3] {
        let c = self.luma * (i32::from(y) - self.black) + 128;
        let (d, e) = (i32::from(u) - 128, i32::from(v) - 128);
        let byte = |x: i32| u8::try_from((x >> 8).clamp(0, 255)).unwrap_or(u8::MAX);
        [
            byte(c + self.rv * e),
            byte(c - self.gu * d - self.gv * e),
            byte(c + self.bu * d),
        ]
    }
}

/// A decoded frame's RGBA, opaque.
fn rgba(picture: &rav1d::Picture) -> Result<Vec<u8>> {
    if picture.bit_depth() != 8 {
        bail!("a frame of {} bits", picture.bit_depth());
    }
    let width = usize::try_from(picture.width())?;
    let height = usize::try_from(picture.height())?;
    let matrix = Matrix::of(picture.matrix_coefficients(), picture.color_range());
    let luma = picture.plane(PlanarImageComponent::Y);
    let stride = usize::try_from(picture.stride(PlanarImageComponent::Y))?;
    // The chroma's planes, their stride and how far they are subsampled across and down.
    let chroma = match picture.pixel_layout() {
        PixelLayout::I400 => None,
        layout => Some((
            picture.plane(PlanarImageComponent::U),
            picture.plane(PlanarImageComponent::V),
            usize::try_from(picture.stride(PlanarImageComponent::U))?,
            usize::from(layout != PixelLayout::I444),
            usize::from(layout == PixelLayout::I420),
        )),
    };
    let mut rgba = Vec::with_capacity(width * height * 4);
    for row in 0..height {
        let ys = luma
            .get(row * stride..row * stride + width)
            .context("a short luma plane")?;
        for (column, &y) in ys.iter().enumerate() {
            let (u, v) = match chroma {
                Some((us, vs, stride, across, down)) => {
                    let at = (row >> down) * stride + (column >> across);
                    let u = us.get(at).context("a short chroma plane")?;
                    let v = vs.get(at).context("a short chroma plane")?;
                    (*u, *v)
                }
                None => (128, 128),
            };
            rgba.extend(matrix.rgb(y, u, v));
            rgba.push(u8::MAX);
        }
    }
    Ok(rgba)
}

/// Where a movie is.
enum Stage {
    /// The wait before it: half a second before a request's first movie, none before the next.
    Before(f64),
    /// Its frames shown, its sound playing.
    Showing,
    /// The wait after it, or for the request's next movie while a key that stopped it is held.
    After,
}

/// A game's play and its change, while a game is played.
type Plays<'w> = (
    Option<Res<'w, State<Play>>>,
    Option<ResMut<'w, NextState<Play>>>,
);

/// The channels that a movie pauses in the menus: the music's and the effects'.
type Sounds<'w> = (
    Option<Res<'w, AudioChannel<music::Channel>>>,
    Option<Res<'w, Audio>>,
);

/// What a movie stood: a running game, or the menus' sound.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Stood {
    Game,
    Sound,
    Nothing,
}

/// A movie's thread's queues, which a resource holds behind a lock.
struct Reading {
    opened: Mutex<Receiver<Result<Opened, String>>>,
    pictures: Mutex<Receiver<Picture>>,
}

/// Starts the thread that reads the movie `name`.
fn reading(name: &str) -> Option<Reading> {
    let path = path(name);
    let (opened, opened_here) = mpsc::channel();
    let (pictures, pictures_here) = mpsc::sync_channel(AHEAD);
    let spawned = thread::Builder::new()
        .name(format!("movie {name}"))
        .spawn(move || read(&path, &opened, &pictures));
    match spawned {
        Ok(_) => Some(Reading {
            opened: Mutex::new(opened_here),
            pictures: Mutex::new(pictures_here),
        }),
        Err(e) => {
            warn!("the movie {name}: {e}");
            None
        }
    }
}

/// The movie that plays, from its request to the end of the wait after the request's last.
#[derive(Resource)]
pub struct Movie {
    /// Its name.
    pub name: String,
    /// The request's movies after it, which play in turn (`0x6A16E0`'s number 13).
    queue: VecDeque<String>,
    stage: Stage,
    /// The real time at which its stage began, in seconds.
    since: f64,
    /// Its sound and its size, once its thread has read them.
    read: Option<Opened>,
    reading: Reading,
    /// The next frame, taken from the queue before its time.
    next: Option<Picture>,
    /// The screen over everything, which takes the pointer from all under it as the original's
    /// player takes every click, and the frames' image on it while the movie shows.
    root: Entity,
    image: Option<(Entity, Handle<Image>)>,
    length: f64,
    stood: Stood,
    /// Whether the screen is black from the request to the end (`Show::black`).
    black: bool,
}

impl Movie {
    /// Ends the movie's frames and sound, if it shows, and begins the wait after it.
    fn stop(&mut self, now: f64) {
        self.stage = Stage::After;
        self.since = now;
    }

    /// Goes on to the request's next movie that reads, at once; false where none is left.
    fn next_movie(&mut self, now: f64) -> bool {
        while let Some(name) = self.queue.pop_front() {
            if let Some(reading) = reading(&name) {
                info!("the movie {name}");
                self.name = name;
                self.reading = reading;
                self.stage = Stage::Before(0.0);
                self.since = now;
                self.read = None;
                self.next = None;
                self.length = 0.0;
                return true;
            }
        }
        false
    }

    /// A movie whose thread has ended before it showed, on the screen `root`, standing nothing.
    #[cfg(test)]
    pub(crate) fn still(root: Entity, black: bool) -> Self {
        let (_, opened) = mpsc::channel();
        let (_, pictures) = mpsc::sync_channel(AHEAD);
        Self {
            name: String::new(),
            queue: VecDeque::new(),
            stage: Stage::Before(BEFORE),
            since: 0.0,
            read: None,
            reading: Reading {
                opened: Mutex::new(opened),
                pictures: Mutex::new(pictures),
            },
            next: None,
            root,
            image: None,
            length: 0.0,
            stood: Stood::Nothing,
            black,
        }
    }
}

/// Whether the movies that play by themselves play: the start's logos, the scripts' and
/// `GameEnd`'s, as the settings' `[movies] show` says (the original's `SHOWMOVIE`), but in a run
/// driven by a script, from afar or for a frame, where they would stand in its way (docs/10 §3);
/// the console's `movies on|off` sets it for the run. The Extras and the console's `movie` play
/// theirs whatever it says, as the original's Extras do (`0x7125B0`).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Automatic(pub bool);

/// The start's logos still to play, once the first menu shows: a run that starts in the main
/// menu has them, one that starts a game at once has not.
#[derive(Resource)]
pub struct Logos;

/// The start's logos, as the original's `initApp` plays them before its main menu (`0x816D6C`, the
/// player's number 13): Ascaron's, the publisher's (`take2` where it is there, else `publisher`;
/// neither shipped) and `oem`.
fn logos(
    mut commands: Commands,
    logos: Option<Res<Logos>>,
    automatic: Res<Automatic>,
    mut shows: MessageWriter<Show>,
) {
    if logos.is_none() {
        return;
    }
    commands.remove_resource::<Logos>();
    if automatic.0 {
        let publisher = if exists("take2") {
            "take2"
        } else {
            "publisher"
        };
        shows.write(Show {
            names: ["ascaron", publisher, "oem"].map(str::to_owned).to_vec(),
            black: true,
        });
    }
}

/// The movie that a script's `InfoPlayer` of `mode` plays (`0x4AE350`): 23 the intro, 24 the
/// Underworld's intro, 25 its extro and 100 to 107 the chapters' `act1` to `act8`; and `GameEnd`'s
/// extro (`scripts::GAME_END`, `0x475C3A`).
fn scripted(mode: i32) -> Option<String> {
    match mode {
        23 => Some("intro".to_owned()),
        24 => Some("introuw".to_owned()),
        25 => Some("extrouw".to_owned()),
        100..=107 => Some(format!("act{}", mode - 99)),
        scripts::GAME_END => Some("extro".to_owned()),
        _ => None,
    }
}

/// Takes the scripts' movies from their `InfoPlayer`s and `GameEnd`s before the texts are shown,
/// and plays them where the movies that play by themselves do.
fn from_scripts(
    mut infos: ResMut<scripts::Infos>,
    automatic: Res<Automatic>,
    mut shows: MessageWriter<Show>,
) {
    let mut names = Vec::new();
    infos.0.retain(|(mode, _)| match scripted(*mode) {
        Some(name) => {
            names.push(name);
            false
        }
        None => true,
    });
    if names.is_empty() {
        return;
    }
    if automatic.0 {
        shows.write(Show::of(names));
    } else {
        info!("the scripts' movies {names:?}: the movies are off");
    }
}

/// Marks the campaigns' extros seen as their play is asked for, which the original writes into
/// `Settings.cfg` before it looks for the file (`0x81A840`, `0x81AA90`): the Extras offer them
/// from then on.
fn extros_seen(
    mut shows: MessageReader<Show>,
    mut settings: ResMut<Settings>,
    mut file: ResMut<SettingsFile>,
) {
    let mut seen = Vec::new();
    for Show { names, .. } in shows.read() {
        for name in names {
            let (flag, key) = match name.as_str() {
                "extro" => (&mut settings.movies.extro, "movies.extro"),
                "extrouw" => (
                    &mut settings.movies.extro_underworld,
                    "movies.extro_underworld",
                ),
                _ => continue,
            };
            if !*flag {
                *flag = true;
                seen.push(key);
            }
        }
    }
    if !seen.is_empty()
        && let Err(e) = file.save(&settings, &seen)
    {
        warn!("movies: keeping the extros seen: {e:#}");
    }
}

/// Starts the movies asked for, unless one plays: the first's thread, the others queued, its
/// screen, and what it stands.
fn start(
    mut shows: MessageReader<Show>,
    movie: Option<Res<Movie>>,
    mut commands: Commands,
    time: Res<Time<Real>>,
    (app, mut still): (Res<State<AppState>>, Option<ResMut<Still>>),
    (play, mut next_play): Plays,
    (music, effects): Sounds,
) {
    let Some(Show { names, black }) = shows.read().last().cloned() else {
        return;
    };
    if movie.is_some() {
        return;
    }
    let mut queue: VecDeque<String> = names
        .into_iter()
        .filter(|name| {
            let there = exists(name);
            if !there {
                info!("the movie {name}: not there, skipped");
            }
            there
        })
        .collect();
    let mut first = None;
    while let Some(name) = queue.pop_front() {
        if let Some(reading) = reading(&name) {
            first = Some((name, reading));
            break;
        }
    }
    let Some((name, reading)) = first else {
        return;
    };
    let stood = match (app.get(), play.as_deref().map(State::get)) {
        (AppState::InGame, Some(Play::Running)) => {
            if let Some(still) = &mut still {
                still.0 = true;
            }
            if let Some(next) = &mut next_play {
                next.set(Play::Paused);
            }
            Stood::Game
        }
        (AppState::InGame, _) => Stood::Nothing,
        _ => {
            if let Some(music) = &music {
                music.pause();
            }
            if let Some(effects) = &effects {
                effects.pause();
            }
            Stood::Sound
        }
    };
    let screen = if black { Color::BLACK } else { Color::NONE };
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(screen),
            GlobalZIndex(LAYER),
        ))
        .id();
    info!("the movie {name}");
    commands.insert_resource(Movie {
        name,
        queue,
        stage: Stage::Before(BEFORE),
        since: time.elapsed_secs_f64(),
        read: None,
        reading,
        next: None,
        root,
        image: None,
        length: 0.0,
        stood,
        black,
    });
}

/// Stops the movie that shows while a key of `Action::SkipMovie` or either button is held
/// (`0x6A0EA0`).
fn stop(
    movie: Option<ResMut<Movie>>,
    time: Res<Time<Real>>,
    keys: Keys,
    buttons: Res<ButtonInput<MouseButton>>,
) {
    if let Some(mut movie) = movie
        && matches!(movie.stage, Stage::Showing)
        && (keys.pressed(Action::SkipMovie) || buttons.any_pressed(STOP_BUTTONS))
    {
        movie.stop(time.elapsed_secs_f64());
    }
}

/// Takes the keys and the buttons pressed while a movie plays from the game and the menus, as
/// the original's player throws their messages away.
fn swallow(
    movie: Option<Res<Movie>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut buttons: ResMut<ButtonInput<MouseButton>>,
) {
    if movie.is_some() {
        keys.clear();
        buttons.clear();
    }
}

/// Plays the movie: the wait before it, then its frames at their times and its sound, until its
/// end; and as it stops, its frames gone, and its screen black no more after the request's last,
/// unless it is black to the end (`Show::black`).
fn advance(
    mut commands: Commands,
    mut movie: ResMut<Movie>,
    time: Res<Time<Real>>,
    (mut images, mut sources): (ResMut<Assets<Image>>, ResMut<Assets<AudioSource>>),
    channel: Res<AudioChannel<Channel>>,
) {
    let now = time.elapsed_secs_f64();
    let movie = &mut *movie;
    match movie.stage {
        Stage::Before(wait) => {
            if movie.read.is_none() {
                let received = movie
                    .reading
                    .opened
                    .get_mut()
                    .unwrap_or_else(PoisonError::into_inner)
                    .try_recv();
                match received {
                    Ok(Ok(read)) => movie.read = Some(read),
                    Ok(Err(e)) => {
                        warn!("the movie {}: {e}", movie.name);
                        movie.stop(now);
                        return;
                    }
                    Err(TryRecvError::Disconnected) => {
                        warn!("the movie {}: its thread ended", movie.name);
                        movie.stop(now);
                        return;
                    }
                    Err(TryRecvError::Empty) => {}
                }
            }
            if now - movie.since < wait {
                return;
            }
            let Some(read) = movie.read.take() else {
                return;
            };
            let size = Extent3d {
                width: read.size.x,
                height: read.size.y,
                depth_or_array_layers: 1,
            };
            let image = images.add(Image::new_fill(
                size,
                TextureDimension::D2,
                &[0, 0, 0, u8::MAX],
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            ));
            let frame = commands
                .spawn((
                    ImageNode::new(image.clone()),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(50.0),
                        top: Val::Px(0.0),
                        margin: UiRect::left(Val::Px(-bar::SCREEN.x / 2.0)),
                        width: Val::Px(bar::SCREEN.x),
                        height: Val::Px(bar::SCREEN.y),
                        ..default()
                    },
                    ChildOf(movie.root),
                ))
                .id();
            commands
                .entity(movie.root)
                .insert(BackgroundColor(Color::BLACK));
            if let Some(sound) = read.sound {
                channel.play(sources.add(AudioSource { sound }));
            }
            movie.image = Some((frame, image));
            movie.length = read.length;
            movie.stage = Stage::Showing;
            movie.since = now;
        }
        Stage::Showing => {
            let clock = now - movie.since;
            let pictures = movie
                .reading
                .pictures
                .get_mut()
                .unwrap_or_else(PoisonError::into_inner);
            let mut newest = None;
            loop {
                let next = movie.next.take().or_else(|| pictures.try_recv().ok());
                match next {
                    Some(picture) if picture.at <= clock => newest = Some(picture),
                    Some(picture) => {
                        movie.next = Some(picture);
                        break;
                    }
                    None => break,
                }
            }
            if let Some(picture) = newest
                && let Some((_, handle)) = &movie.image
                && let Some(mut image) = images.get_mut(handle)
            {
                image.data = Some(picture.rgba);
            }
            if clock >= movie.length {
                movie.stop(now);
            }
        }
        Stage::After => {
            if let Some((frame, _)) = movie.image.take() {
                channel.stop();
                commands.entity(frame).despawn();
                if movie.queue.is_empty() && !movie.black {
                    commands
                        .entity(movie.root)
                        .insert(BackgroundColor(Color::NONE));
                }
            }
        }
    }
}

/// Once the keys of `Action::SkipMovie` are up after a movie: the request's next movie, or, a
/// quarter of a second after the last, the end, its screen gone and what it stood going on.
fn end(
    mut commands: Commands,
    mut movie: ResMut<Movie>,
    time: Res<Time<Real>>,
    keys: Keys,
    (play, mut next_play): Plays,
    (music, effects): Sounds,
) {
    let now = time.elapsed_secs_f64();
    if !matches!(movie.stage, Stage::After) || keys.pressed(Action::SkipMovie) {
        return;
    }
    if movie.next_movie(now) {
        return;
    }
    if now - movie.since < AFTER {
        return;
    }
    commands.entity(movie.root).despawn();
    commands.remove_resource::<Movie>();
    match movie.stood {
        Stood::Game => {
            if play.is_some_and(|play| *play.get() == Play::Paused)
                && let Some(next) = &mut next_play
            {
                next.set(Play::Running);
            }
        }
        Stood::Sound => {
            if let Some(music) = &music {
                music.resume();
            }
            if let Some(effects) = &effects {
                effects.resume();
            }
        }
        Stood::Nothing => {}
    }
}

/// The movies: their requests, the start's logos, the scripts', their play, their keys, and the
/// world's controls held back while one plays.
pub struct MoviesPlugin;

impl Plugin for MoviesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Show>()
            .add_audio_channel::<Channel>()
            .insert_resource(Automatic(true))
            .add_systems(OnEnter(AppState::Menu), logos)
            .add_systems(
                PreUpdate,
                (stop, swallow).chain().after(InputSystems).before(Hold),
            )
            .add_systems(
                Update,
                (
                    from_scripts.before(talk::notices),
                    (
                        extros_seen
                            .run_if(resource_exists::<Settings>)
                            .run_if(resource_exists::<SettingsFile>),
                        start,
                    ),
                    (advance, end).chain().run_if(resource_exists::<Movie>),
                )
                    .chain(),
            )
            .hold_control(|movie: Option<Res<Movie>>| movie.is_none());
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::ScheduleSystem;
    use bevy::prelude::Messages;

    use super::*;

    #[test]
    fn the_standard_definitions_colours_come_out_of_their_yuv() {
        let bt601 = Matrix::of(MatrixCoefficients::Unspecified, YUVRange::Limited);
        assert_eq!(bt601.rgb(16, 128, 128), [0, 0, 0]);
        assert_eq!(bt601.rgb(235, 128, 128), [255, 255, 255]);
        // BT.601's red, green and blue in the limited range.
        assert_eq!(bt601.rgb(81, 90, 240), [255, 0, 0]);
        assert_eq!(bt601.rgb(145, 54, 34), [0, 255, 1]);
        assert_eq!(bt601.rgb(41, 240, 110), [0, 0, 255]);
        let full = Matrix::of(MatrixCoefficients::BT709, YUVRange::Full);
        assert_eq!(full.rgb(0, 128, 128), [0, 0, 0]);
        assert_eq!(full.rgb(255, 128, 128), [255, 255, 255]);
    }

    /// `tests/video/colours.webm`: three frames of 64 × 48 at 25 a second, red, green and blue,
    /// and 0.12 s of a tone of 440 Hz, that FFmpeg encoded with SVT-AV1 and libopus as the
    /// converter's `build` does.
    #[test]
    fn a_movie_decodes_into_its_frames_and_its_sound() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/video/colours.webm");
        let (opened, video) = demux(std::fs::read(path).unwrap()).unwrap();
        assert_eq!(opened.size, UVec2::new(64, 48));
        // The tone's 0.12 s, the encoder's delay dropped, and its last Opus frame's padding of
        // under 20 ms, which the WebM demuxer keeps: the sound outlasts the frames' 0.12 s.
        assert!((0.12..0.14).contains(&opened.length), "{}", opened.length);
        let sound = opened.sound.unwrap();
        assert_eq!(sound.sample_rate, 48_000);
        assert!(
            (5_760..6_720).contains(&sound.frames.len()),
            "{}",
            sound.frames.len()
        );
        let (sender, receiver) = mpsc::sync_channel(AHEAD);
        let decoding = thread::spawn(move || video.decode(&sender));
        let pictures: Vec<Picture> = receiver.iter().collect();
        decoding.join().unwrap().unwrap();
        let times: Vec<f64> = pictures.iter().map(|p| p.at).collect();
        assert_eq!(times, [0.0, 0.04, 0.08]);
        for (picture, colour) in pictures.iter().zip([[255, 0, 0], [0, 255, 0], [0, 0, 255]]) {
            assert_eq!(picture.rgba.len(), 64 * 48 * 4);
            // The middle pixel, within the encoder's loss.
            let middle = (24 * 64 + 32) * 4;
            for (got, want) in picture.rgba[middle..middle + 3].iter().zip(colour) {
                assert!(
                    got.abs_diff(want) <= 12,
                    "{:?}",
                    &picture.rgba[middle..middle + 4]
                );
            }
            assert_eq!(picture.rgba[middle + 3], 255);
        }
    }

    #[test]
    fn a_movie_stopped_ends_its_decoding() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/video/colours.webm");
        let (_, video) = demux(std::fs::read(path).unwrap()).unwrap();
        let (sender, receiver) = mpsc::sync_channel(0);
        drop(receiver);
        video.decode(&sender).unwrap();
    }

    #[test]
    fn the_scripts_modes_name_their_movies() {
        let named = |mode| scripted(mode);
        assert_eq!(named(23).as_deref(), Some("intro"));
        assert_eq!(named(24).as_deref(), Some("introuw"));
        assert_eq!(named(25).as_deref(), Some("extrouw"));
        assert_eq!(named(100).as_deref(), Some("act1"));
        assert_eq!(named(107).as_deref(), Some("act8"));
        assert_eq!(named(scripts::GAME_END).as_deref(), Some("extro"));
        for mode in [0, 1, 5, 22, 26, 99, 108] {
            assert_eq!(named(mode), None, "{mode}");
        }
    }

    /// The movies asked for in an app of `system` alone, run once.
    fn shown<M>(app: &mut App, system: impl IntoScheduleConfigs<ScheduleSystem, M>) -> Vec<Show> {
        app.add_message::<Show>().add_systems(Update, system);
        app.update();
        app.world_mut()
            .resource_mut::<Messages<Show>>()
            .drain()
            .collect()
    }

    #[test]
    fn the_scripts_movies_play_by_themselves_and_leave_their_texts() {
        for on in [true, false] {
            let mut app = App::new();
            app.insert_resource(scripts::Infos(vec![
                (1, "res:17631".to_owned()),
                (101, String::new()),
                (scripts::GAME_END, String::new()),
            ]))
            .insert_resource(Automatic(on));
            let shows = shown(&mut app, from_scripts);
            assert_eq!(
                app.world().resource::<scripts::Infos>().0,
                [(1, "res:17631".to_owned())]
            );
            let wanted = if on {
                vec![Show::of(vec!["act2".to_owned(), "extro".to_owned()])]
            } else {
                Vec::new()
            };
            assert_eq!(shows, wanted, "{on}");
        }
    }

    #[test]
    fn the_start_shows_its_logos_once_unless_the_movies_are_off() {
        for on in [true, false] {
            let mut app = App::new();
            app.insert_resource(Logos).insert_resource(Automatic(on));
            let shows = shown(&mut app, logos);
            // Black from the request, as nothing shows before them.
            let wanted = if on {
                vec![Show {
                    names: vec![
                        "ascaron".to_owned(),
                        "publisher".to_owned(),
                        "oem".to_owned(),
                    ],
                    black: true,
                }]
            } else {
                Vec::new()
            };
            assert_eq!(shows, wanted, "{on}");
            assert!(!app.world().contains_resource::<Logos>());
            app.update();
            assert!(
                app.world_mut()
                    .resource_mut::<Messages<Show>>()
                    .drain()
                    .next()
                    .is_none()
            );
        }
    }

    #[test]
    fn a_black_requests_screen_stays_black_after_its_last_movie() {
        for black in [true, false] {
            let mut app = App::new();
            app.add_plugins(bevy::MinimalPlugins)
                .init_resource::<Assets<Image>>()
                .init_resource::<Assets<AudioSource>>()
                .init_resource::<AudioChannel<Channel>>()
                .add_systems(Update, advance);
            let root = app.world_mut().spawn(BackgroundColor(Color::BLACK)).id();
            let frame = app.world_mut().spawn(ChildOf(root)).id();
            let mut movie = Movie::still(root, black);
            movie.stage = Stage::After;
            movie.image = Some((frame, Handle::default()));
            app.insert_resource(movie);
            app.update();
            assert!(app.world().get_entity(frame).is_err(), "{black}");
            let screen = app.world().get::<BackgroundColor>(root).map(|c| c.0);
            let wanted = if black { Color::BLACK } else { Color::NONE };
            assert_eq!(screen, Some(wanted), "{black}");
        }
    }
}
