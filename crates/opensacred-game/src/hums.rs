//! The loops that the spells' and arts' objects hold while they live (#1173; docs/re/sound.md,
//! "Spells and arts"): an object's particle system starts its loop as it starts (`0x6934E0`),
//! keeps its channel (`+0xB0`) and stops it as it ends; each at the object, at the volume 1 and
//! the sample's own rate. An entity that carries a [`Hum`] loops its sound at its place, panned and
//! fading by it, until it goes or the hum is taken from it.

use bevy::prelude::*;

use crate::frame::Playing;
use crate::game::AppState;
use crate::hero::Body;

/// A loop, of a sound id, at its entity while it carries it.
#[derive(Component, Clone, Copy, Debug)]
pub struct Hum(pub u32);

/// The loops that play, each at its entity.
#[derive(Resource, Default)]
struct Hums(Vec<(Entity, Handle<bevy_kira_audio::AudioInstance>)>);

/// Starts each new hum's loop, keeps each at its entity's place, and stops a loop whose entity
/// goes or no longer carries it.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn hum(
    mut hums: ResMut<Hums>,
    humming: Query<(Entity, &Hum, &GlobalTransform)>,
    heroes: Query<&GlobalTransform, With<Body>>,
    mut sfx: ResMut<crate::sfx::Sfx>,
    heard: Res<crate::sfx::Listener>,
    assets: Res<AssetServer>,
    audio: Res<bevy_kira_audio::Audio>,
    mut instances: ResMut<Assets<bevy_kira_audio::AudioInstance>>,
) {
    use bevy_kira_audio::prelude::{AudioControl, AudioTween};
    hums.0.retain(|(entity, handle)| {
        let keep = humming.contains(*entity);
        if !keep && let Some(mut instance) = instances.get_mut(handle) {
            instance.stop(AudioTween::default());
        }
        keep
    });
    for (entity, &Hum(sound), _) in &humming {
        if hums.0.iter().any(|(playing, _)| *playing == entity) {
            continue;
        }
        let Some((id, file)) = sfx.with_id(sound) else {
            continue;
        };
        let sample = sfx.sample(id, file, &assets);
        let handle = audio
            .play(sample)
            .looped()
            .with_volume(crate::sfx::decibels(0.0))
            .handle();
        debug!("hums: {entity}'s loop {sound} starts");
        hums.0.push((entity, handle));
    }
    let hero = heroes.iter().next().map(GlobalTransform::translation);
    for (entity, handle) in &hums.0 {
        let (Ok((_, _, at)), Some(mut instance)) =
            (humming.get(*entity), instances.get_mut(handle))
        else {
            continue;
        };
        let (volume, pan) = crate::sfx::placed_at(at.translation(), hero, *heard);
        instance.set_decibels(crate::sfx::decibels(volume), AudioTween::default());
        instance.set_panning(pan, AudioTween::default());
    }
}

/// Stops the loops as the game ends.
fn hush(mut hums: ResMut<Hums>, mut instances: ResMut<Assets<bevy_kira_audio::AudioInstance>>) {
    for (_, handle) in hums.0.drain(..) {
        if let Some(mut instance) = instances.get_mut(&handle) {
            instance.stop(bevy_kira_audio::prelude::AudioTween::default());
        }
    }
}

/// The objects' loops.
pub struct HumsPlugin;

impl Plugin for HumsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Hums>()
            .add_systems(OnExit(AppState::InGame), hush)
            .add_systems(
                Update,
                hum.in_set(Playing).run_if(in_state(AppState::InGame)),
            );
    }
}
