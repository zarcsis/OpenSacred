//! Whether the models about the hero are drawn, which a script's `wait ready` waits for beyond the
//! hero's clip and the sectors (#3909): every model within [`RADIUS`] of the hero has its scene
//! spawned and its meshes their model materials ([`models::Spawned`], no [`models::Unplaced`]),
//! but a creature whose model failed to load; and no render pipeline is still compiling
//! ([`PipelinesReady`]), as a mesh whose pipeline is not made yet is not drawn. Both hold for
//! [`FRAMES`] frames running, so that a pipeline asked for by a model converted in the last frame
//! counts. A model that never comes is waited for [`AT_MOST`] at most, and named in the log.

use std::time::Duration;

use bevy::asset::LoadState;
use bevy::extract::{ExtractSchedule, MainWorld};
use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::render_resource::PipelineCache;

use crate::hero::Body;
use crate::models::{self, Ordered};

/// How far from the hero the models waited for stand, in world units: past the screen's corners
/// at the widest zoom.
const RADIUS: f32 = 1_600.0;
/// The frames running for which the models and the pipelines must be ready.
const FRAMES: u32 = 3;
/// How long the models are waited for once nothing else is, before the wait gives up on them.
const AT_MOST: Duration = Duration::from_secs(10);

/// Whether no render pipeline waits to be made, as the render world last saw it.
#[derive(Resource, Default)]
pub struct PipelinesReady(bool);

/// The frames running for which the models about the hero have been drawn, since the hero animates
/// and its sectors are in place; and since when, so far, they have not been.
#[derive(Resource, Default)]
pub struct Drawn {
    frames: u32,
    since: Option<Duration>,
    gave_up: bool,
}

impl Drawn {
    /// Whether the models about the hero are drawn, or have been waited for [`AT_MOST`].
    pub fn ready(&self) -> bool {
        self.frames >= FRAMES || self.gave_up
    }
}

/// Tells the main world whether a render pipeline still waits to be made.
fn pipelines(mut main: ResMut<MainWorld>, cache: Res<PipelineCache>) {
    let ready = cache.waiting_pipelines().next().is_none();
    if let Some(mut pipelines) = main.get_resource_mut::<PipelinesReady>()
        && pipelines.0 != ready
    {
        pipelines.0 = ready;
    }
}

/// The models about the hero that are not drawn yet: within [`RADIUS`], their scene not spawned or
/// their meshes without their model materials, but a creature's whose model failed to load.
fn waiting(world: &mut World) -> Vec<Entity> {
    let Some(hero) = world
        .query_filtered::<&GlobalTransform, With<Body>>()
        .iter(world)
        .next()
        .map(GlobalTransform::translation)
    else {
        return Vec::new();
    };
    let mut roots = world.query_filtered::<(
        Entity,
        &GlobalTransform,
        Has<models::Spawned>,
        Has<models::Unplaced>,
        Option<&crate::npcs::Npc>,
    ), With<Ordered>>();
    let server = world.resource::<AssetServer>().clone();
    roots
        .iter(world)
        .filter(|(_, at, ..)| (at.translation() - hero).xz().length() <= RADIUS)
        .filter(|(_, _, spawned, unplaced, npc)| {
            let failed = npc.is_some_and(|npc| {
                matches!(server.load_state(npc.gltf_id()), LoadState::Failed(_))
            });
            !failed && (!spawned || *unplaced)
        })
        .map(|(entity, ..)| entity)
        .collect()
}

/// Counts the frames running for which the models about the hero are drawn, once the hero animates
/// and its sectors are in place; gives up on them, naming them in the log, after [`AT_MOST`].
fn watch(world: &mut World) {
    let hero = world
        .query_filtered::<&GlobalTransform, With<Body>>()
        .iter(world)
        .next()
        .map(GlobalTransform::translation);
    let placed = world
        .get_resource::<crate::hero::Ready>()
        .is_some_and(|ready| ready.0)
        && hero
            .zip(world.get_resource::<crate::sectors::Streamer>())
            .is_some_and(|(hero, streamer)| streamer.settled(hero));
    let left = if placed { waiting(world) } else { Vec::new() };
    let drawn = placed && left.is_empty() && world.resource::<PipelinesReady>().0;
    let now = world.resource::<Time<Real>>().elapsed();
    let mut state = world.resource_mut::<Drawn>();
    if !placed || drawn {
        state.since = None;
        state.gave_up = false;
        state.frames = if drawn {
            state.frames.saturating_add(1)
        } else {
            0
        };
        return;
    }
    state.frames = 0;
    let since = *state.since.get_or_insert(now);
    if !state.gave_up && now.saturating_sub(since) >= AT_MOST {
        state.gave_up = true;
        warn!(
            "drawn: {} models about the hero not drawn after {} s: {left:?}",
            left.len(),
            AT_MOST.as_secs()
        );
    }
}

/// Whether the models about the hero are drawn.
pub struct DrawnPlugin;

impl Plugin for DrawnPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PipelinesReady>()
            .init_resource::<Drawn>()
            .add_systems(Last, watch);
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(ExtractSchedule, pipelines);
        }
    }
}
