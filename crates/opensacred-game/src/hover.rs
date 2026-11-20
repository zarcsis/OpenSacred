//! The hover's highlight (#1460; docs/re/cursor.md, "The point, the size, the tint and the
//! buttons"): the foe under the cursor, or the container under the hand, takes the original's
//! `+0x14` bit `0x20000000`, which the world's drawing reads (`0x62FDE5`): its light's ambient
//! share, 0.8 of its colour otherwise and 0.3 for a creature's (#3108), pulses from 1.0 down to
//! 0.2 and back every 0.4 s (`0x642FD0`), by which `standard.wesl` lights its model.

use bevy::prelude::*;

use crate::ai::Mind;
use crate::cursors::Highlit;
use crate::frame::Playing;
use crate::models::{Anchors, Ordered};

/// The pulse (`0x62FE0C`): the share is `|0.8 − (t mod 400)·0.004| + 0.2` at the time `t` in
/// milliseconds.
const PERIOD_MS: u128 = 400;
const STEP: f32 = 0.004;
const HIGH: f32 = 0.8;
const LOW: f32 = 0.2;

/// The ambient share of a model's light not under the cursor, and of a type of the creatures'
/// category (`items.pak +0x2E` 3, docs/02 §6.1), by `0x62FDE9`.
const PLAIN: f32 = 0.8;
const CREATURES_PLAIN: f32 = 0.3;
const CREATURES: u8 = 3;

/// The ambient share of a highlighted model's light at `ms` milliseconds.
fn share(ms: u128) -> f32 {
    let into = f32::from(u16::try_from(ms % PERIOD_MS).unwrap_or(0));
    (HIGH - into * STEP).abs() + LOW
}

/// The ambient share of a model's light not under the cursor, by its type's category (none for
/// an object that is no creature, a container's).
fn plain(category: Option<u8>) -> f32 {
    if category == Some(CREATURES) {
        CREATURES_PLAIN
    } else {
        PLAIN
    }
}

/// The slot of the model lit so, if one is.
#[derive(Resource, Default)]
struct Lit(Option<u32>);

/// The slot of the model of `entity`: its own root's, else the first of its descendants'.
fn slot_of(entity: Entity, roots: &Query<&Ordered>, children: &Query<&Children>) -> Option<u32> {
    roots.get(entity).ok().and_then(Ordered::slot).or_else(|| {
        children
            .iter_descendants(entity)
            .find_map(|child| roots.get(child).ok().and_then(Ordered::slot))
    })
}

/// Lights the model under the cursor by the pulse against its own share, and the one before it
/// plainly again.
fn pulse(
    mut lit: ResMut<Lit>,
    mut anchors: ResMut<Anchors>,
    (time, tables): (Res<Time>, Option<Res<crate::drops::Tables>>),
    highlit: Res<Highlit>,
    (roots, children, minds): (Query<&Ordered>, Query<&Children>, Query<&Mind>),
) {
    let slot = highlit
        .0
        .and_then(|entity| slot_of(entity, &roots, &children));
    if lit.0 != slot {
        if let Some(old) = lit.0 {
            anchors.light(old, 0.0, 0.0);
        }
        lit.0 = slot;
    }
    if let Some((slot, entity)) = slot.zip(highlit.0) {
        let category = minds
            .get(entity)
            .ok()
            .zip(tables.as_deref())
            .map(|(mind, tables)| tables.category(mind.thinks.creature));
        anchors.light(slot, share(time.elapsed().as_millis()), plain(category));
    }
}

/// The hover's highlight.
pub struct HoverPlugin;

impl Plugin for HoverPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Lit>()
            .add_systems(Update, pulse.in_set(Playing));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_light_pulses_from_full_down_to_a_fifth_and_back_every_0_4_s() {
        let near = |a: f32, b: f32| (a - b).abs() < 1e-5;
        assert!(near(share(0), 1.0));
        assert!(near(share(100), 0.6));
        assert!(near(share(200), 0.2));
        assert!(near(share(300), 0.6));
        assert!(near(share(400), 1.0));
    }

    #[test]
    fn a_creature_is_lit_plainly_at_0_3_and_any_other_model_at_0_8() {
        assert_eq!(plain(Some(3)), 0.3);
        // The heroes and the dummy (2), the waters' crab and fish (0x20), the chests (4).
        for other in [None, Some(2), Some(0x20), Some(4)] {
            assert_eq!(plain(other), 0.8);
        }
    }
}
