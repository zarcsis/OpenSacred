//! The crows and the bats over the camera (#1073; docs/re/weather.md, "The butterflies and the
//! birds"): in a sector whose fliers' byte names them, one flies over the camera every 20 to 180 s
//! (`weather` and `opensacred_rules::weather::Weather::fliers` time it), but by night
//! (`0x41BAB0`), while a place of a pool of eight is free (`0x637060`). It starts 1,124 units
//! behind the camera or 1,024 ahead along the world's x at even odds, 768 units to either side
//! along y at random, and flies straight to a point as far on the other side, 240 units of the
//! models above the ground's zero, at 115 of them a second, playing its fly's loop (the motion
//! slot 179), for 20 s (`0x41F180`, `0x41F400`, `0x41F1B0`). It calls first 5 to 30 s after it
//! starts, then every 10 to 30 s, until a roll of `rand()/32767 · 100` above 60 silences it for
//! good: a crow `ANIMAL_CROW01`, a bat `ANIMAL_BAT04` or `05` at even odds, in the middle.

use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;
use opensacred_rules::spawning::Rng;
use opensacred_rules::weather::{BAT, CROW};

use crate::daynight::{Clock, Phase};
use crate::frame::{Playing, Unpaused};
use crate::game::{AppState, Data};
use crate::hero::{self, Body};
use crate::sfx::{self, Of, Roll};
use crate::{iso, models, motions, npcs};

/// A flier due over the camera, of its creature type (`weather`).
#[derive(Message, Clone, Copy, Debug)]
pub struct Due(pub u32);

/// The pool's places (`0x637060`: eight of `0x34` bytes at `+0x828C`).
const POOL: usize = 8;
/// A flier's life, in seconds (`+0x28`).
const LIFE: f32 = 20.0;
/// How far behind the camera and ahead of it along the world's x a flier starts or ends, and how
/// far to either side along y, in world units.
const BEHIND: f32 = 1_124.0;
const AHEAD: f32 = 1_024.0;
const SIDE: u32 = 1_536;
/// Its height above the ground's zero and its speed, in the models' units.
const HEIGHT: f32 = 240.0;
const SPEED: f32 = 115.0;
/// The motion slot of its fly's loop (`CROW_FLY_LOOP`, `BATX_FLY_LOOP`).
const FLY: u16 = 179;
/// Its calls: the crow's, the bat's two.
const CROW_CALL: u32 = 126;
const BAT_CALLS: [u32; 2] = [103, 104];
/// A call's roll above which the flier calls no more.
const SILENT_ABOVE: u32 = 60;
/// The seed of the fliers' `rand()` (**estimate**: any).
const SEED: usize = 0x2C;

/// The fliers' models and their fly's clips, by their creature types, and the dice.
#[derive(Resource)]
struct Looks {
    crow: Option<(String, String)>,
    bat: Option<(String, String)>,
    dice: Rng,
}

impl Looks {
    /// A number from 0 to 1 at random, as the client's `rand()` / 32767.
    fn share(&mut self) -> f32 {
        f32::from(u16::try_from(self.dice.below(0x8000)).unwrap_or(0)) / 32_767.0
    }

    /// The seconds to a flier's next call: its first 5 to 30 s on, the rest 10 to 30 s.
    fn wait(&mut self, first: bool) -> f32 {
        let (span, least) = if first { (25.0, 5.0) } else { (20.0, 10.0) };
        (self.share() * span + least).trunc()
    }
}

/// A flier over the camera: its type, its velocity in world units a second, the seconds it still
/// lives and to its next call, and whether it calls no more.
#[derive(Component)]
struct Flier {
    kind: u32,
    velocity: Vec3,
    life: f32,
    call: f32,
    silent: bool,
    /// Its glTF until its scene is given, and its fly's clip.
    gltf: Option<Handle<Gltf>>,
    clip: String,
}

/// Its fly's clip, which its animation player plays as it comes.
#[derive(Component)]
struct Flying(Handle<AnimationClip>);

/// The fliers' models of a new game: the crow's and the bat's types' `.GRN` and their clips of
/// the slot [`FLY`].
fn begin(mut commands: Commands, data: Option<Res<Data>>) {
    let Some(data) = data else {
        return;
    };
    let shared = &data.0;
    let looks = (|| -> anyhow::Result<_> {
        let items = shared.tables.items()?;
        let models = npcs::model_files(&shared.assets)?;
        let look = |kind: u32| {
            let item = items.iter().find(|item| item.slot == kind)?;
            let model = models.get(&item.name.to_ascii_uppercase())?.clone();
            let slots = motions::Slots::read(&shared.assets.join(&model)).ok()?;
            Some((model, slots.slot(FLY)?.to_owned()))
        };
        Ok((look(CROW), look(BAT)))
    })();
    let (crow, bat) = looks.unwrap_or_else(|e| {
        warn!("fliers: no models: {e:#}");
        (None, None)
    });
    commands.insert_resource(Looks {
        crow,
        bat,
        dice: Rng::of(0, 0, SEED),
    });
}

/// Sends each flier due, but by night and while the pool has room, over the camera, which follows
/// the hero.
fn send(
    mut commands: Commands,
    mut due: MessageReader<Due>,
    looks: Option<ResMut<Looks>>,
    clock: Option<Res<Clock>>,
    fliers: Query<(), With<Flier>>,
    heroes: Query<&Transform, With<Body>>,
    assets: Res<AssetServer>,
) {
    let (Some(mut looks), Some(at)) = (looks, heroes.iter().next()) else {
        due.clear();
        return;
    };
    let mut count = fliers.iter().count();
    for &Due(kind) in due.read() {
        if clock
            .as_ref()
            .is_some_and(|clock| clock.phase() == Phase::Night)
            || count >= POOL
        {
            continue;
        }
        let look = if kind == CROW {
            &looks.crow
        } else {
            &looks.bat
        };
        let Some((model, clip)) = look.clone() else {
            continue;
        };
        let (x, y) = (at.translation.x, at.translation.z);
        let side = |looks: &mut Looks| {
            let across = looks.dice.below(SIDE);
            y + f32::from(u16::try_from(across).unwrap_or(0)) - 768.0
        };
        let (from, to) = if looks.dice.below(2) == 1 {
            (x - BEHIND, x + AHEAD)
        } else {
            (x + AHEAD, x - BEHIND)
        };
        let start = Vec3::new(from, HEIGHT * iso::MODEL_SCALE, side(&mut looks));
        let end = Vec3::new(to, start.y, side(&mut looks));
        let along = (end - start).normalize_or_zero();
        let call = looks.wait(true);
        debug!("fliers: a {kind} flies from {start} toward {end}");
        commands.spawn((
            Flier {
                kind,
                velocity: along * SPEED * iso::MODEL_SCALE,
                life: LIFE,
                call,
                silent: false,
                gltf: Some(assets.load(model)),
                clip,
            },
            Transform::from_translation(start)
                .with_rotation(hero::facing(along))
                .with_scale(Vec3::splat(iso::MODEL_SCALE)),
            Visibility::default(),
            models::Ordered::default(),
        ));
        count += 1;
    }
}

/// Gives a flier its model's scene and its fly's clip once its glTF is loaded.
fn dress(
    mut commands: Commands,
    mut fliers: Query<(Entity, &mut Flier)>,
    gltfs: Res<Assets<Gltf>>,
) {
    for (entity, mut flier) in &mut fliers {
        let Some(gltf) = flier.gltf.as_ref().and_then(|handle| gltfs.get(handle)) else {
            continue;
        };
        let scene = gltf.scenes.first().cloned();
        let clip = gltf.named_animations.get(flier.clip.as_str()).cloned();
        let source = flier.gltf.take();
        if let (Some(scene), Some(source)) = (scene, source) {
            commands
                .entity(entity)
                .insert((WorldAssetRoot(scene), crate::models::Source(source)));
        }
        if let Some(clip) = clip {
            commands.entity(entity).insert(Flying(clip));
        }
    }
}

/// Plays a flier's fly's loop on its animation player as it comes.
fn animate(
    mut commands: Commands,
    fliers: Query<(Entity, &Flying)>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer, Without<AnimationGraphHandle>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    for (flier, flying) in &fliers {
        for entity in children.iter_descendants(flier) {
            if let Ok(mut player) = players.get_mut(entity) {
                let (graph, node) = AnimationGraph::from_clip(flying.0.clone());
                player.play(node).repeat();
                commands
                    .entity(entity)
                    .insert(AnimationGraphHandle(graphs.add(graph)));
            }
        }
    }
}

/// Moves each flier on, calls as its time comes, and takes it away as its life ends.
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    looks: Option<ResMut<Looks>>,
    mut fliers: Query<(Entity, &mut Flier, &mut Transform)>,
    mut sounds: MessageWriter<sfx::Play>,
) {
    let Some(mut looks) = looks else {
        return;
    };
    let dt = time.delta_secs();
    for (entity, mut flier, mut transform) in &mut fliers {
        flier.life -= dt;
        if flier.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation += flier.velocity * dt;
        if flier.silent {
            continue;
        }
        flier.call -= dt;
        if flier.call > 0.0 {
            continue;
        }
        let roll = looks.dice.below(0x8000) * 100 / 32_767;
        if roll > SILENT_ABOVE {
            flier.silent = true;
            continue;
        }
        let call = if flier.kind == CROW {
            CROW_CALL
        } else {
            BAT_CALLS[usize::try_from(looks.dice.below(2)).unwrap_or(0)]
        };
        sounds.write(sfx::Play {
            sound: Of::Id(call),
            at: None,
            rate: (0.5, Roll::None),
        });
        flier.call = looks.wait(false);
    }
}

/// The fliers of a game ended.
fn end(mut commands: Commands, fliers: Query<Entity, With<Flier>>) {
    for flier in &fliers {
        commands.entity(flier).despawn();
    }
    commands.remove_resource::<Looks>();
}

/// The crows and the bats over the camera.
pub struct FliersPlugin;

impl Plugin for FliersPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Due>()
            .add_systems(OnEnter(AppState::InGame), begin)
            .add_systems(OnExit(AppState::InGame), end)
            .add_systems(
                Update,
                (send, dress, animate, fly)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fliers_calls_wait_their_ranges() {
        let mut looks = Looks {
            crow: None,
            bat: None,
            dice: Rng::of(0, 0, SEED),
        };
        for _ in 0..200 {
            let first = looks.wait(true);
            assert!((5.0..=30.0).contains(&first), "{first}");
            let next = looks.wait(false);
            assert!((10.0..=30.0).contains(&next), "{next}");
        }
    }
}
