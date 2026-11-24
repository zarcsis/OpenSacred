//! The eye lasers' impact (909, `FX_LASERBEAM_IMPACT`, #3152; docs/re/effects.md, "The eye
//! lasers' impact (909)"): where a big demon's beam reaches its target (`creature_casts`, the
//! data 49), a spray of fifty yellow sparks bursts at the target's neck, else its root, placed
//! there once (the FX event `0x79`, the flags `0x40`), and falls within half a second
//! (`opensacred_rules::particles::laser_impact`, drawn by `puffs`), lighting the world by 200 and
//! the models about it by its Direct3D light of (1, 0.8, 0.2), the priority 2 (`fx_lights`).
//!
//! Not followed: the sparks' copies drawn back along their way, its draw's own trail layer.

use bevy::prelude::*;
use opensacred_rules::particles::{LASER_IMPACT_D3D, laser_impact};
use opensacred_rules::spawning::Rng;

use crate::frame::Playing;
use crate::game::AppState;

/// A beam's impact at the creature `on` it reaches.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Impact {
    pub on: Entity,
}

/// The neck's bone (3), else the root.
const NECK: [&str; 1] = ["Bip01 Neck"];
/// The priority of its Direct3D light (`0x7A6510`).
const PRIORITY: u8 = 2;

/// The impacts' dice (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x909))
    }
}

/// Bursts each impact's sparks at its creature's neck, else its root, with their lights.
fn burst(
    mut commands: Commands,
    mut impacts: MessageReader<Impact>,
    places: Query<&GlobalTransform>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    mut dice: ResMut<Dice>,
) {
    for impact in impacts.read() {
        let Some(point) = crate::making::bone(impact.on, &NECK, &children, (&bones, &worn))
            .or_else(|| places.get(impact.on).ok().map(GlobalTransform::translation))
        else {
            continue;
        };
        debug!("laser impacts: at {} ({point})", impact.on);
        let sparks = crate::puffs::spawn(&mut commands, laser_impact(&mut dice.0), point, None);
        commands.spawn(crate::fx_lights::light(
            LASER_IMPACT_D3D,
            PRIORITY,
            point,
            sparks,
            (None, Vec3::ZERO),
        ));
    }
}

/// The eye lasers' impacts.
pub struct LaserImpactsPlugin;

impl Plugin for LaserImpactsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Impact>()
            .init_resource::<Dice>()
            .add_systems(
                Update,
                burst
                    .in_set(Playing)
                    .after(crate::frame::Step::Npcs)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
