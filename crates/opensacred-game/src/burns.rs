//! The burns of the effect 62 (#760; docs/re/arts.md, "The Daemon's"), which the Daemon's 1101 and
//! the like give: their caster keeps them, one a target, each striking its target about once a
//! second, at once the first time, with its damage in its kind for its seconds
//! (`opensacred_rules::arts::Burn`), the fight's hit of fixed damages, which always lands; a burn
//! ends with its seconds or with its target gone or dead.

use bevy::prelude::*;
use opensacred_rules::arts::Burn;

use crate::fight::{self, Health};
use crate::frame::{Playing, Step};
use crate::npcs::Dies;

/// A caster's burns: each its target and its burn.
#[derive(Component, Clone, Debug, Default)]
pub struct Burns(Vec<(Entity, Burn)>);

impl Burns {
    /// Adds `burn` on `target`, refused where a burn of the caster's holds on that target
    /// already (the adder `0x584DDB`): whether it was added.
    pub fn add(&mut self, target: Entity, burn: Burn) -> bool {
        if self.0.iter().any(|&(on, _)| on == target) {
            return false;
        }
        self.0.push((target, burn));
        true
    }
}

/// Steps each caster's burns and strikes as they come due (`0x562730` at `0x563786`).
fn burn(
    time: Res<Time>,
    mut casters: Query<(Entity, &mut Burns)>,
    targets: Query<&Health, Without<Dies>>,
    mut hits: MessageWriter<fight::Hit>,
) {
    let dt = time.delta_secs();
    for (caster, mut burns) in &mut casters {
        burns.0.retain_mut(|(target, burn)| {
            let Ok(health) = targets.get(*target) else {
                return false;
            };
            if health.current == 0 {
                return false;
            }
            if burn.step(dt) {
                debug!("burns: {caster} burns {target} for {:?}", burn.damages());
                hits.write(fight::Hit {
                    attacker: caster,
                    target: *target,
                    share: 1.0,
                    ranged: false,
                    art: None,
                    fixed: Some(burn.damages()),
                    sure: true,
                    poison: false,
                });
            }
            !burn.over()
        });
    }
}

/// The burns.
pub struct BurnsPlugin;

impl Plugin for BurnsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, burn.after(Step::Npcs).in_set(Playing));
    }
}
