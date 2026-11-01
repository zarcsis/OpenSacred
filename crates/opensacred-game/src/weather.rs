//! The world's weather (#1072, docs/re/weather.md): the rain, the fog and the snow that the sector
//! the hero is in chooses by its weather entries (`world.ron`), on timers of real time that every
//! sector shares, and their levels moving toward what the choice wants as the game runs
//! (`opensacred_rules::weather`), the rolls `Sacred.exe`'s (`tables/weather_rolls.ron`). `skies`
//! draws and plays it (#1073); the overlay (F7) and `state` show it, and the console's
//! `setrain`, `setfog` and `setsnow` set it, as the original's do (`0x614230`).

use std::collections::HashMap;
use std::path::Path;

use bevy::prelude::{
    App, IntoScheduleConfigs, MessageWriter, OnEnter, OnExit, Plugin, Query, Real, Res, ResMut,
    Resource, Time, Transform, Update, With, World,
};
use opensacred_data::tables::WEATHER_ROLLS;
use opensacred_data::world::{SIDE, SectorWeather, WeatherKind};
use opensacred_rules::spawning::Rng;
use opensacred_rules::weather::{self, Event};

use crate::frame::Playing;
use crate::game::{AppState, Data};
use crate::{files, hero, paths};

/// The seed of the storm's lightning's `rand()` (**estimate**: any).
const SEED: usize = 0x5C;

/// The rolls of `tables/weather_rolls.ron`, none for assets converted before it (#1072): every
/// time is then infinite, so that no weather comes.
#[derive(Resource, Default)]
pub struct Rolls(Vec<u8>);

impl Rolls {
    /// Reads `tables/weather_rolls.ron` of `assets`.
    pub fn read(assets: &Path) -> anyhow::Result<Self> {
        let path = assets.join("tables").join(WEATHER_ROLLS);
        if !files::exists(&path) {
            bevy::log::warn!("weather: no {}; no weather comes", path.display());
            return Ok(Self::default());
        }
        let text = files::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
        let rolls: Vec<u8> =
            ron::from_str(&text).map_err(|e| anyhow::anyhow!("parsing {}: {e}", path.display()))?;
        Ok(Self(rolls))
    }
}

/// The weather of a game played: the renderer's state of it, the sectors' entries by their
/// places and ids, and the lightning's dice.
#[derive(Resource)]
pub struct Weather {
    pub state: weather::Weather,
    /// Each sector's id, weather entries and fliers' byte.
    sectors: HashMap<(u32, u32), (u32, Vec<SectorWeather>, u8)>,
    dice: Rng,
    /// The lightnings come since `skies` last drew them.
    pub lightnings: u32,
    /// The quakes sent since `skies` last heard them.
    pub quakes: u32,
}

impl Weather {
    /// The weather of a new game in the world `world`, of no weather yet.
    pub fn new(world: &opensacred_data::world::World) -> Self {
        Self {
            state: weather::Weather::default(),
            sectors: world
                .sectors
                .iter()
                .map(|entry| {
                    (
                        (entry.x, entry.y),
                        (entry.id, entry.weather.clone(), entry.fliers),
                    )
                })
                .collect(),
            dice: crate::seed::dice(SEED),
            lightnings: 0,
            quakes: 0,
        }
    }

    /// The console's `setrain`, `setfog` and `setsnow` (`0x614230`): their kinds' events of the
    /// value `value`, at most 100.
    pub fn set(&mut self, event: Event) {
        if matches!(event, Event::Quake(value) if value > 0) {
            self.quakes += 1;
        }
        self.state.event(event, &mut self.dice);
    }

    /// How many entries of the butterflies the sector at `at` has (`butterflies`).
    pub fn butterflies(&self, at: (u32, u32)) -> u32 {
        self.sectors.get(&at).map_or(0, |(_, entries, _)| {
            let count = entries
                .iter()
                .filter(|entry| entry.kind == WeatherKind::Butterflies)
                .count();
            u32::try_from(count).unwrap_or(0)
        })
    }

    /// What the overlay shows of it: the rain's drops, the fog's alpha and the snow's flakes now,
    /// none while no kind is on.
    pub fn levels(&self) -> Option<(u32, u32, u32)> {
        let state = &self.state;
        (state.flags != 0).then_some((state.rain.now, state.fog.now, state.snow.now))
    }
}

/// The weather's choice and its levels, at each frame of a game (`0x625290`, `0x41A4F0`): the
/// sector the hero is in chooses by its entries at the time of real time, which runs on in the
/// pause, and the levels move by the game's time, which the pause holds; a flier due over the
/// camera by the sector's byte goes to `fliers`.
fn choose(
    real: Res<Time<Real>>,
    mut due: MessageWriter<crate::fliers::Due>,
    time: Res<Time>,
    rolls: Res<Rolls>,
    weather: Option<ResMut<Weather>>,
    heroes: Query<&Transform, With<hero::Body>>,
) {
    let (Some(mut weather), Some(at)) = (weather, heroes.iter().next()) else {
        return;
    };
    let weather = &mut *weather;
    let side = u32::try_from(SIDE).unwrap_or(64);
    let (x, y) = paths::cell_of(at.translation.x, at.translation.z);
    if let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y))
        && let Some((id, entries, fliers)) = weather.sectors.get(&(x / side, y / side))
    {
        let now = real.elapsed_secs_f64() * 1_000.0;
        weather
            .state
            .choose(now, *id, entries, &rolls.0, &mut weather.dice);
        if let Some(kind) = weather.state.fliers(now / 1_000.0, *fliers, &rolls.0) {
            due.write(crate::fliers::Due(kind));
        }
    }
    let lightnings = weather.state.update(time.delta_secs(), &mut weather.dice);
    if lightnings > 0 {
        bevy::log::debug!("weather: a lightning");
        weather.lightnings += lightnings;
    }
}

/// A new game's weather, of none yet.
fn begin(world: &mut World) {
    let Some(data) = world.get_resource::<Data>() else {
        return;
    };
    let weather = Weather::new(&data.0.world);
    world.insert_resource(weather);
}

fn end(world: &mut World) {
    world.remove_resource::<Weather>();
}

pub struct WeatherPlugin;

impl Plugin for WeatherPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), begin)
            .add_systems(OnExit(AppState::InGame), end)
            .add_systems(Update, choose.in_set(Playing));
    }
}
