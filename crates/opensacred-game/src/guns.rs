//! The Dwarf's mortar shells (#759; docs/re/arts.md, "The Dwarf's"): his 1086 throws one to its
//! point (`making`), and [`opensacred_rules::arts::MORTAR_DELAY`] later, whatever the distance, it
//! strikes every hostile creature alive within [`opensacred_rules::arts::MORTAR_REACH`] and its
//! size of the point with its damages (the impact 1020's, `0x5B05E3`), the fight's hit of fixed
//! damages, which lands. The grenade and its blast are not drawn.

use bevy::prelude::*;
use opensacred_rules::arts::{MORTAR_DELAY, MORTAR_REACH};

use crate::ai::Mind;
use crate::crowd::InTheWay;
use crate::fight::{self, Health};
use crate::frame::{Playing, Step};
use crate::game::AppState;
use crate::hero::Body;
use crate::npcs::Dies;
use crate::terrain::Level;

/// A shell on its way: its thrower, its point, the level it falls on, its damages and how long
/// until it strikes.
#[derive(Component)]
pub struct Shell {
    by: Entity,
    at: Vec2,
    level: Level,
    damages: [f32; 4],
    left: f32,
}

impl Shell {
    /// A shell of `by` to the point `at` on `level`, of `damages`, gone as the game ends.
    pub fn thrown(by: Entity, at: Vec2, level: Level, damages: [f32; 4]) -> impl Bundle {
        (
            Self {
                by,
                at,
                level,
                damages,
                left: MORTAR_DELAY,
            },
            DespawnOnExit(AppState::InGame),
        )
    }
}

/// Strikes about each shell's point as its time comes.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn explode(
    mut commands: Commands,
    time: Res<Time>,
    mut shells: Query<(Entity, &mut Shell)>,
    creatures: Query<
        (
            Entity,
            &Transform,
            &InTheWay,
            &Mind,
            &Health,
            Option<&crate::arrows::Size>,
        ),
        (Without<Body>, Without<Dies>),
    >,
    mut hits: MessageWriter<fight::Hit>,
) {
    for (entity, mut shell) in &mut shells {
        shell.left -= time.delta_secs();
        if shell.left > 0.0 {
            continue;
        }
        for (target, at, way, mind, health, size) in &creatures {
            let size = size.map_or(crate::arrows::size(0), |size| size.0);
            if way.creature
                && way.level == shell.level
                && mind.hostile()
                && health.current > 0
                && (at.translation.xz() - shell.at).length() < MORTAR_REACH + size
            {
                debug!("guns: {entity} of {} strikes {target}", shell.by);
                hits.write(fight::Hit {
                    attacker: shell.by,
                    target,
                    share: 1.0,
                    ranged: false,
                    art: None,
                    fixed: Some(shell.damages),
                    sure: true,
                    poison: false,
                });
            }
        }
        commands.entity(entity).despawn();
    }
}

/// The Dwarf's mortar shells.
pub struct GunsPlugin;

impl Plugin for GunsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, explode.after(Step::Npcs).in_set(Playing));
    }
}
