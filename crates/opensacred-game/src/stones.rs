//! The flesh to stone's sight (822, `cParticleSystem_stoned`, #1092; docs/re/effects.md, "The flesh
//! to stone"): the creature that the stone holds, a hero's cast's or a creature's, stands in a block
//! of stone (`freeze_stone.grn` in its item's texture `FREEZE_STONE01`) at its feet, turned about
//! the upright at random, following it while the hold lasts (the flag `0x200`), and its model wears
//! the stone's skin as long (`skins`, the state 9).

use bevy::prelude::*;
use opensacred_rules::spawning::Rng;
use opensacred_rules::spells::{STONED_MODEL, STONED_TEXTURE};

use crate::frame::{Playing, Unpaused};
use crate::game::{AppState, Data};
use crate::iso;
use crate::npcs::Retexture;
use crate::skins::{Asked, Overlay};

/// A stone to make about its creature for its seconds ([`make`]).
#[derive(Component)]
struct Unmade {
    target: Entity,
    seconds: f32,
}

/// A stone that stands: its creature and its seconds left.
#[derive(Component)]
struct Stone {
    target: Entity,
    left: f32,
}

/// The stone's model and its texture, once looked for.
#[derive(Resource, Default)]
struct Looks(Option<(Option<String>, Option<String>)>);

/// The stones' turns' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x822))
    }
}

/// Holds `target` in stone for `seconds`: its block of stone, and its skin of stone (the action 7,
/// the event `0x109`).
pub(crate) fn stone(commands: &mut Commands, target: Entity, seconds: f32) {
    commands.spawn((Unmade { target, seconds }, DespawnOnExit(AppState::InGame)));
    commands
        .entity(target)
        .insert(Asked(Overlay::Stone, seconds));
}

/// Gives each new stone its model at its creature's feet, turned at random (`0x786120`), in the
/// stone's texture.
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    places: Query<&GlobalTransform>,
    (data, assets, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
) {
    for (entity, unmade) in &unmade {
        let (model, texture) = looks
            .0
            .get_or_insert_with(|| {
                let Some(data) = data.as_deref() else {
                    return (None, None);
                };
                let model = crate::npcs::model_files(&data.0.assets)
                    .ok()
                    .and_then(|files| files.get(STONED_MODEL).cloned());
                if model.is_none() {
                    warn!("stones: no {STONED_MODEL}; the stones are not drawn");
                }
                (
                    model,
                    crate::fx::texture_path(&data.0.assets, STONED_TEXTURE),
                )
            })
            .clone();
        let (Some(model), Ok(at)) = (model, places.get(unmade.target)) else {
            commands.entity(entity).despawn();
            continue;
        };
        // `rand()` / 32767 of a whole turn.
        let roll = f32::from(u16::try_from(dice.0.below(0x8000)).unwrap_or(0)) / 32_767.0;
        let turn = roll * std::f32::consts::TAU;
        let mut made = commands.entity(entity);
        made.remove::<Unmade>().insert((
            crate::models::scene(&assets, &model),
            Transform::from_translation(at.translation())
                .with_rotation(Quat::from_rotation_y(turn))
                .with_scale(Vec3::splat(iso::MODEL_SCALE)),
            crate::models::Ordered::default(),
            Stone {
                target: unmade.target,
                left: unmade.seconds,
            },
        ));
        if let Some(texture) = &texture {
            made.insert(Retexture::load(&assets, texture));
        }
        debug!(
            "stones: {} stands in stone for {:.1} s",
            unmade.target, unmade.seconds
        );
    }
}

/// Keeps each stone at its creature's feet (`0x5A9C50`), and takes it off as its seconds end or its
/// creature goes (`0x786000`).
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut stones: Query<(Entity, &mut Stone, &mut Transform)>,
    places: Query<&GlobalTransform, Without<Stone>>,
) {
    for (entity, mut stone, mut transform) in &mut stones {
        stone.left -= time.delta_secs();
        let Ok(at) = places.get(stone.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        if stone.left <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation = at.translation();
    }
}

/// The flesh to stone's sight.
pub struct StonesPlugin;

impl Plugin for StonesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_systems(
                Update,
                (make, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
