//! The debug overlay (docs/10 §5): F3 shows and hides, in the upper left, the frame rate and
//! times, where the hero is and what it does, the clock, the zoom, the sectors, the NPCs and the
//! objects, the entities, the version, the GPU and the window, refreshed four times a second in
//! Bevy's monospaced default font. Whether it shows is kept in the settings (`[debug] overlay`).
//! The same values, a [`Snapshot`], are what the command `state` and the remote protocol give.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::text::FontSize;
use bevy::window::{PrimaryWindow, WindowMode};
use serde::Serialize;

use crate::daynight::{Clock, Phase};
use crate::fight::{Experience, Health};
use crate::game::AppState;
use crate::hero::{Body, Gait, Hero, Ready};
use crate::npcs::{Npc, Object};
use crate::pause::Play;
use crate::sectors::Streamer;
use crate::settings::{Settings, SettingsFile};
use crate::{iso, terrain};

/// How often the overlay's text changes.
const REFRESH: Duration = Duration::from_millis(250);
/// The window of the frame times.
const SECOND: Duration = Duration::from_secs(1);
/// The overlay's font size on the original's 768-pixel-high screen (`hud`).
const FONT_SIZE: f32 = 10.0;
/// The overlay's colours, in the frame's sRGB values (`hud`).
const BACK: Color = Color::linear_rgba(0.0, 0.0, 0.0, 0.6);
const TEXT: Color = Color::linear_rgb(0.85, 0.85, 0.85);

/// The frames of the last second: when each ended and how long it took, by the real time.
#[derive(Resource, Default)]
pub struct Frames(VecDeque<(Instant, Duration)>);

impl Frames {
    /// Adds a frame that ended at `now` after `delta`, forgetting those older than a second.
    fn add(&mut self, now: Instant, delta: Duration) {
        self.0.push_back((now, delta));
        while self
            .0
            .front()
            .is_some_and(|(at, _)| now.duration_since(*at) > SECOND)
        {
            self.0.pop_front();
        }
    }

    /// The frames a second, the mean frame time and the slowest, in milliseconds.
    fn rates(&self) -> (f32, f32, f32) {
        let count = self.0.len();
        if count == 0 {
            return (0.0, 0.0, 0.0);
        }
        let total: Duration = self.0.iter().map(|(_, delta)| *delta).sum();
        let slowest = self
            .0
            .iter()
            .map(|(_, delta)| *delta)
            .max()
            .unwrap_or_default();
        let mean = total.as_secs_f32() / count as f32;
        let fps = if mean > 0.0 { 1.0 / mean } else { 0.0 };
        (fps, mean * 1000.0, slowest.as_secs_f32() * 1000.0)
    }
}

fn measure(time: Res<Time<Real>>, mut frames: ResMut<Frames>) {
    frames.add(Instant::now(), time.delta());
}

/// The hero as the overlay shows it.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct HeroState {
    /// The global cell it stands on, and its sector.
    pub cell: (i64, i64),
    pub sector: (i64, i64),
    /// The ground, or a building's inside layer (`sector x, sector y, layer`).
    pub inside: Option<(u32, u32, u16)>,
    /// The cell it goes to, if any.
    pub target: Option<(i64, i64)>,
    /// The level of the cell it goes to, when a click chose one: `ground`, or `inside X,Y:L`.
    pub onto: Option<String>,
    /// `walk` or `run`.
    pub gait: &'static str,
    pub moving: bool,
}

/// What the overlay shows: the process's state, and the game's if one is played.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub version: &'static str,
    /// `menu`, `loading`, `game` or `paused`.
    pub state: &'static str,
    pub fps: f32,
    pub frame_ms: f32,
    pub slowest_ms: f32,
    /// The window's size in physical pixels and its mode.
    pub window: Option<(u32, u32, &'static str)>,
    /// The GPU adapter and its backend.
    pub gpu: Option<String>,
    pub hero: Option<HeroState>,
    /// The clock as `HH:MM`, and the phase of the day.
    pub clock: Option<(String, &'static str)>,
    pub zoom: Option<f32>,
    /// The sectors spawned, and those being built.
    pub sectors: Option<(usize, usize)>,
    pub npcs: usize,
    pub objects: usize,
    pub entities: usize,
    /// The health of the creature nearest the hero, its most and its level.
    pub nearest: Option<(u32, u32, u32)>,
    /// The hero's level and experience (`fight::Experience`), and its health and most.
    pub experience: Option<(u32, u64)>,
    pub health: Option<(u32, u32)>,
    /// The drops nearest the hero (`drops`): what each is, its item, its amount, how far it is
    /// and whether it lies on the ground.
    pub drops: Vec<(String, u32, u32, f32, bool)>,
    /// The hero's combat arts and what is still to run of each, in seconds (`arts::HeroArts`,
    /// #509).
    pub arts: Vec<(u16, f32)>,
    /// Whether the hero animates and every sector near it is in place.
    pub ready: bool,
}

/// A level as the overlay names it: `ground`, or `inside X,Y:L` (the inside layer `L` of sector
/// `X,Y`).
pub(crate) fn place(level: Option<(u32, u32, u16)>) -> String {
    match level {
        Some((x, y, layer)) => format!("inside {x},{y}:{layer}"),
        None => "ground".into(),
    }
}

/// The global cell of world point `p`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a cell of the world, a whole number well within i64"
)]
fn cell_of(p: Vec3) -> (i64, i64) {
    (
        (p.x / iso::CELL).floor() as i64,
        (p.z / iso::CELL).floor() as i64,
    )
}

impl Snapshot {
    /// The overlay's lines.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        let mut head = format!("OpenSacred {}", self.version);
        if let Some(gpu) = &self.gpu {
            head.push_str(&format!(" | {gpu}"));
        }
        if let Some((width, height, mode)) = self.window {
            head.push_str(&format!(" | {width}x{height} {mode}"));
        }
        lines.push(head);
        lines.push(format!(
            "{:6.1} fps {:6.2} ms  slowest {:6.2} ms  {}",
            self.fps, self.frame_ms, self.slowest_ms, self.state
        ));
        if let Some(hero) = &self.hero {
            let place = place(hero.inside);
            let mut line = format!(
                "hero cell {},{} sector {},{} {place}, {}, {}",
                hero.cell.0,
                hero.cell.1,
                hero.sector.0,
                hero.sector.1,
                hero.gait,
                if hero.moving { "moving" } else { "standing" }
            );
            if let Some((x, y)) = hero.target {
                line.push_str(&format!(" to {x},{y}"));
                if let Some(onto) = &hero.onto {
                    line.push_str(&format!(" {onto}"));
                }
            }
            lines.push(line);
        }
        let mut world = Vec::new();
        if let Some((time, phase)) = &self.clock {
            world.push(format!("{time} {phase}"));
        }
        if let Some(zoom) = self.zoom {
            world.push(format!("zoom {zoom:.2}"));
        }
        if let Some((spawned, building)) = self.sectors {
            world.push(format!("sectors {spawned} +{building}"));
        }
        if !world.is_empty() {
            lines.push(world.join("  "));
            lines.push(format!(
                "NPCs {}  objects {}  entities {}{}",
                self.npcs,
                self.objects,
                self.entities,
                if self.ready { "" } else { "  not ready" }
            ));
            if let Some((current, most, level)) = self.nearest {
                lines.push(format!(
                    "nearest creature  health {current}/{most}  level {level}"
                ));
            }
            match (self.health, self.experience) {
                (Some((current, most)), Some((level, experience))) => lines.push(format!(
                    "hero level {level}  health {current}/{most}  experience {experience}"
                )),
                (None, Some((level, experience))) => {
                    lines.push(format!("hero level {level}  experience {experience}"));
                }
                _ => {}
            }
            if !self.drops.is_empty() {
                let drops: Vec<String> = self
                    .drops
                    .iter()
                    .map(|(kind, item, amount, distance, lies)| {
                        let amount = if *amount > 1 {
                            format!(" x{amount}")
                        } else {
                            String::new()
                        };
                        let how = if *lies { "lying" } else { "flying" };
                        format!("{kind} {item}{amount} at {distance:.0} {how}")
                    })
                    .collect();
                lines.push(format!("drops {}", drops.join(", ")));
            }
            if !self.arts.is_empty() {
                let arts: Vec<String> = self
                    .arts
                    .iter()
                    .map(|(art, remaining)| {
                        if *remaining <= 0.01 {
                            format!("{art} ready")
                        } else {
                            format!("{art} {remaining:.1} s")
                        }
                    })
                    .collect();
                lines.push(format!("arts {}", arts.join(", ")));
            }
        }
        lines
    }
}

/// How many of the drops nearest the hero the state lists.
const NEAREST_DROPS: usize = 5;

/// What a [`Snapshot`] is gathered from.
#[derive(SystemParam)]
pub struct Probe<'w, 's> {
    frames: Res<'w, Frames>,
    app: Option<Res<'w, State<AppState>>>,
    play: Option<Res<'w, State<Play>>>,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    adapter: Option<Res<'w, RenderAdapterInfo>>,
    hero: Option<Res<'w, Hero>>,
    bodies: Query<'w, 's, (&'static Transform, &'static Body)>,
    clock: Option<Res<'w, Clock>>,
    cameras: Query<'w, 's, &'static Projection, With<Camera3d>>,
    streamer: Option<Res<'w, Streamer>>,
    ready: Option<Res<'w, Ready>>,
    npcs: Query<'w, 's, (), With<Npc>>,
    healths: Query<'w, 's, (&'static Transform, &'static Health), Without<Body>>,
    experience: Option<Res<'w, Experience>>,
    hero_health: Query<'w, 's, &'static Health, With<Body>>,
    objects: Query<'w, 's, (), With<Object>>,
    entities: Query<'w, 's, ()>,
    drops: Query<'w, 's, (&'static Transform, &'static crate::drops::Lying)>,
    arts: Option<Res<'w, crate::arts::HeroArts>>,
}

impl Probe<'_, '_> {
    pub fn snapshot(&self) -> Snapshot {
        let (fps, frame_ms, slowest_ms) = self.frames.rates();
        let state = match (self.app.as_deref().map(State::get), self.play.as_deref()) {
            (Some(AppState::InGame), Some(play)) if *play.get() == Play::Paused => "paused",
            (Some(AppState::InGame), _) => "game",
            (Some(AppState::Loading), _) => "loading",
            _ => "menu",
        };
        let window = self.windows.iter().next().map(|window| {
            let mode = match window.mode {
                WindowMode::Windowed => "windowed",
                WindowMode::BorderlessFullscreen(_) => "borderless",
                WindowMode::Fullscreen(..) => "full screen",
            };
            (
                window.resolution.physical_width(),
                window.resolution.physical_height(),
                mode,
            )
        });
        let gpu = self
            .adapter
            .as_deref()
            .map(|info| format!("{} ({:?})", info.name, info.backend));
        let hero =
            self.bodies
                .iter()
                .next()
                .zip(self.hero.as_deref())
                .map(|((transform, body), hero)| {
                    let cell = cell_of(transform.translation);
                    let side = i64::try_from(opensacred_data::world::SIDE).unwrap_or(64);
                    let level: terrain::Level = body.level();
                    HeroState {
                        cell,
                        sector: (cell.0.div_euclid(side), cell.1.div_euclid(side)),
                        inside: level,
                        target: body.target().map(cell_of),
                        onto: body.onto().map(place),
                        gait: match hero.gait {
                            Gait::Walk => "walk",
                            Gait::Run => "run",
                        },
                        moving: body.moving().is_some(),
                    }
                });
        let clock = self.clock.as_deref().map(|clock| {
            let minutes = clock.minutes();
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a minute of the day, 0 to 1,439"
            )]
            let minute = minutes.floor() as u32;
            let phase = match clock.phase() {
                Phase::Dawn => "dawn",
                Phase::Day => "day",
                Phase::Dusk => "dusk",
                Phase::Night => "night",
            };
            (format!("{:02}:{:02}", minute / 60, minute % 60), phase)
        });
        let zoom = self
            .hero
            .as_ref()
            .and(self.cameras.iter().next())
            .and_then(|projection| match projection {
                Projection::Orthographic(ortho) => Some(ortho.scale),
                _ => None,
            });
        Snapshot {
            version: env!("OPENSACRED_VERSION"),
            state,
            fps,
            frame_ms,
            slowest_ms,
            window,
            gpu,
            hero,
            clock,
            zoom,
            sectors: self
                .streamer
                .as_deref()
                .map(|streamer| (streamer.sectors().count(), streamer.building())),
            npcs: self.npcs.iter().count(),
            objects: self.objects.iter().count(),
            entities: self.entities.iter().count(),
            nearest: self.bodies.iter().next().and_then(|(hero, _)| {
                self.healths
                    .iter()
                    .min_by(|a, b| {
                        let d = |t: &Transform| (t.translation - hero.translation).length_squared();
                        d(a.0).total_cmp(&d(b.0))
                    })
                    .map(|(_, health)| (health.current, health.most, health.level))
            }),
            experience: self
                .experience
                .as_deref()
                .map(|experience| (experience.level, experience.points)),
            health: self
                .hero_health
                .iter()
                .next()
                .map(|health| (health.current, health.most)),
            drops: self
                .bodies
                .iter()
                .next()
                .map(|(hero, _)| {
                    crate::drops::nearest(
                        hero.translation,
                        self.drops
                            .iter()
                            .map(|(at, lying)| (at.translation, *lying)),
                        NEAREST_DROPS,
                    )
                })
                .unwrap_or_default(),
            arts: self
                .arts
                .as_deref()
                .map(|arts| {
                    arts.arts
                        .entries
                        .iter()
                        .map(|e| (e.art, e.remaining))
                        .collect()
                })
                .unwrap_or_default(),
            ready: self.ready.as_deref().is_some_and(|ready| ready.0)
                && self
                    .bodies
                    .iter()
                    .next()
                    .zip(self.streamer.as_deref())
                    .is_some_and(|((transform, _), streamer)| {
                        streamer.settled(transform.translation)
                    }),
        }
    }
}

/// The overlay's panel.
#[derive(Component)]
struct Panel;

fn visibility(shown: bool) -> Visibility {
    if shown {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

fn spawn_panel(mut commands: Commands, settings: Res<Settings>) {
    commands.spawn((
        Panel,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(6.0),
            top: Val::Px(6.0),
            padding: UiRect::all(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(BACK),
        GlobalZIndex(i32::MAX - 2),
        visibility(settings.debug.overlay),
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(FONT_SIZE),
            ..default()
        },
        TextColor(TEXT),
    ));
}

/// F3: the overlay shown or hidden, kept in the settings.
fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<Settings>,
    mut file: ResMut<SettingsFile>,
) {
    if !keys.just_pressed(KeyCode::F3) {
        return;
    }
    settings.debug.overlay = !settings.debug.overlay;
    if let Err(e) = file.save(&settings, &["debug.overlay"]) {
        warn!("settings: {e:#}");
    }
}

/// Shows the overlay as the settings say, and its lines four times a second.
fn draw(
    settings: Res<Settings>,
    time: Res<Time<Real>>,
    mut since: Local<Duration>,
    probe: Probe,
    mut panel: Single<(&mut Visibility, &mut Text), With<Panel>>,
) {
    let (visibility_, text) = &mut *panel;
    if settings.is_changed() {
        **visibility_ = visibility(settings.debug.overlay);
        *since = REFRESH;
    }
    if !settings.debug.overlay {
        return;
    }
    *since += time.delta();
    if *since < REFRESH {
        return;
    }
    *since = Duration::ZERO;
    text.0 = probe.snapshot().lines().join("\n");
}

/// The debug overlay on F3 and the frame times it shows.
pub struct OverlayPlugin;

impl Plugin for OverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Frames>()
            .add_systems(First, measure)
            .add_systems(Startup, spawn_panel)
            .add_systems(Update, (toggle, draw).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frames_of_the_last_second_give_the_rate_the_mean_and_the_slowest() {
        let mut frames = Frames::default();
        let start = Instant::now();
        // A slow frame that ends at 0.5 s, then 49 of 20 ms ending from 0.68 s to 1.64 s and one
        // of 40 ms at 1.68 s: the second up to 1.68 s leaves the first behind.
        frames.add(
            start + Duration::from_millis(500),
            Duration::from_millis(100),
        );
        for i in 0..49 {
            frames.add(
                start + Duration::from_millis(680 + 20 * i),
                Duration::from_millis(20),
            );
        }
        frames.add(
            start + Duration::from_millis(1680),
            Duration::from_millis(40),
        );
        let (fps, mean, slowest) = frames.rates();
        assert_eq!(frames.0.len(), 50);
        assert!((mean - 20.4).abs() < 1e-3, "{mean}");
        assert!((fps - 1000.0 / 20.4).abs() < 1e-2, "{fps}");
        assert!((slowest - 40.0).abs() < 1e-3);
    }

    #[test]
    fn the_lines_show_the_game_when_one_is_played() {
        let mut snapshot = Snapshot {
            version: "0.6.2",
            state: "menu",
            fps: 60.0,
            frame_ms: 16.67,
            slowest_ms: 18.0,
            window: Some((1920, 1080, "windowed")),
            gpu: Some("GPU (Vulkan)".into()),
            ..Snapshot::default()
        };
        assert_eq!(
            snapshot.lines(),
            [
                "OpenSacred 0.6.2 | GPU (Vulkan) | 1920x1080 windowed",
                "  60.0 fps  16.67 ms  slowest  18.00 ms  menu",
            ]
        );
        snapshot.state = "game";
        snapshot.hero = Some(HeroState {
            cell: (3236, 2510),
            sector: (50, 39),
            inside: None,
            target: Some((3240, 2512)),
            onto: Some("inside 50,39:2".into()),
            gait: "run",
            moving: true,
        });
        snapshot.clock = Some(("08:13".into(), "day"));
        snapshot.zoom = Some(1.0);
        snapshot.sectors = Some((9, 0));
        snapshot.npcs = 34;
        snapshot.objects = 120;
        snapshot.entities = 18_234;
        snapshot.ready = true;
        assert_eq!(
            snapshot.lines()[2..],
            [
                "hero cell 3236,2510 sector 50,39 ground, run, moving to 3240,2512 inside 50,39:2",
                "08:13 day  zoom 1.00  sectors 9 +0",
                "NPCs 34  objects 120  entities 18234",
            ]
        );
    }

    #[test]
    fn f3_shows_the_overlay_and_the_settings_keep_it() {
        let dir =
            std::env::temp_dir().join(format!("opensacred-game-overlay-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("settings.toml");
        let (file, settings) = SettingsFile::read(Some(path.clone()));
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, OverlayPlugin))
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(file)
            .insert_resource(settings);
        app.update();
        let panel = |app: &mut App| {
            let mut panel = app
                .world_mut()
                .query_filtered::<(&Visibility, &Text), With<Panel>>();
            let (visibility, text) = panel.single(app.world()).unwrap();
            (*visibility != Visibility::Hidden, text.0.clone())
        };
        assert!(!panel(&mut app).0);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F3);
        app.update();
        let (shown, text) = panel(&mut app);
        assert!(shown);
        assert!(text.starts_with("OpenSacred "), "{text}");
        assert!(text.ends_with(" menu"), "{text}");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "[debug]\noverlay = true\n"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_point_lies_in_its_cell() {
        assert_eq!(cell_of(iso::ground(3236.5, 2510.9)), (3236, 2510));
        assert_eq!(cell_of(iso::ground(-0.5, 0.0)), (-1, 0));
    }
}
