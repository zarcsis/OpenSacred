//! The scripts' `Partikel`s (99, `0x4A8BB0`; #1828; docs/re/netscript.md, "`Partikel`"): each
//! makes the generic system 856 and starts it with the FX event `0x50` in the record's colour and
//! of its variant, at the creature that its first name names (`hero` the hero), else at the centre
//! of its cell on the ground, else nowhere. The game makes the variants 1 and 4, a ring of sparks
//! rising about the place, the variant 4 with `SOUND_FX_SPAWN` (`opensacred_rules::particles::
//! generic_ring`), drawn as the other bursts (`puffs`); at a creature the ring stands where it
//! stood, as the original places the system once at the creature's root.

use bevy::prelude::*;
use opensacred_rules::particles::{self, SPAWN_RING, SPAWN_SOUND};
use opensacred_rules::spawning::Rng;

use crate::frame::Playing;
use crate::hero::Body;
use crate::npcs::ScriptName;
use crate::scripts::Partikel;
use crate::sfx::{Of, Play, Roll};
use crate::terrain::Heights;

/// The dice of the rings' sparks.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x1828))
    }
}

/// Where a `Partikel` makes its system: at the hero for `hero`, at the creature of its name, the
/// first in the world, else at its cell's centre on the ground, else nowhere (`0x4A9091`).
fn place(
    partikel: &Partikel,
    hero: Option<Vec3>,
    creature: impl Fn(&str) -> Option<Vec3>,
    ground: impl Fn(Vec3) -> Vec3,
) -> Option<Vec3> {
    let named = partikel.at.as_deref().and_then(|name| match name {
        "hero" => hero,
        name => creature(name),
    });
    named.or_else(|| {
        let (x, y) = partikel.cell?;
        Some(ground(crate::iso::ground(x as f32 + 0.5, y as f32 + 0.5)))
    })
}

/// Makes the scripts' `Partikel`s.
fn make(
    mut commands: Commands,
    mut made: MessageReader<Partikel>,
    (heights, mut dice): (Res<Heights>, ResMut<Dice>),
    heroes: Query<&Transform, With<Body>>,
    creatures: Query<(&ScriptName, &Transform), Without<Body>>,
    mut sounds: MessageWriter<Play>,
) {
    for partikel in made.read() {
        let hero = heroes.iter().next().map(|at| at.translation);
        let creature = |name: &str| {
            creatures
                .iter()
                .find(|(named, _)| named.0 == name)
                .map(|(_, at)| at.translation)
        };
        let ground = |at: Vec3| {
            let y = heights.standing(at.x, at.z, None).unwrap_or(at.y);
            Vec3::new(at.x, y, at.z)
        };
        let Some(at) = place(partikel, hero, creature, ground) else {
            debug!("script_particles: {partikel:?} finds no place");
            continue;
        };
        let Some(burst) = particles::generic_ring(partikel.variant, partikel.colour, &mut dice.0)
        else {
            debug!(
                "script_particles: the variant {} is not followed",
                partikel.variant
            );
            continue;
        };
        let entity = crate::puffs::spawn(&mut commands, burst, at, None);
        if partikel.variant == SPAWN_RING {
            sounds.write(Play {
                sound: Of::Id(SPAWN_SOUND),
                at: Some(entity),
                rate: (0.5, Roll::None),
            });
        }
        debug!(
            "script_particles: the variant {} in {:08X} at {at}",
            partikel.variant, partikel.colour
        );
    }
}

/// The scripts' `Partikel`s.
pub struct ScriptParticlesPlugin;

impl Plugin for ScriptParticlesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .add_systems(Update, make.after(crate::frame::Step::Npcs).in_set(Playing));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_partikel_stands_at_its_creature_else_at_its_cell() {
        let partikel = |at: Option<&str>, cell| Partikel {
            at: at.map(str::to_owned),
            cell,
            colour: 0xFFFF_0000,
            variant: 1,
        };
        let hero = Some(Vec3::new(1.0, 2.0, 3.0));
        let creature = |name: &str| (name == "res:d1dorf_01").then_some(Vec3::X);
        let ground = |at: Vec3| at + Vec3::Y;
        // A villager of `OMO-1D1Change`, the hero, a cell of `EffektTimer`; a name that names
        // nothing falls back to the cell, and with neither nothing.
        assert_eq!(
            place(
                &partikel(Some("res:d1dorf_01"), None),
                hero,
                creature,
                ground
            ),
            Some(Vec3::X)
        );
        assert_eq!(
            place(&partikel(Some("hero"), None), hero, creature, ground),
            hero
        );
        let cell = Some(crate::iso::ground(6287.5, 1437.5) + Vec3::Y);
        assert_eq!(
            place(&partikel(None, Some((6287, 1437))), hero, creature, ground),
            cell
        );
        assert_eq!(
            place(
                &partikel(Some("res:gone"), Some((6287, 1437))),
                hero,
                creature,
                ground
            ),
            cell
        );
        assert_eq!(
            place(&partikel(Some("res:gone"), None), hero, creature, ground),
            None
        );
    }

    #[test]
    fn a_partikels_numbers_are_its_cell_colour_and_variant() {
        // `particles { _01 = "", _0B = { 3940, 1706, 44287, 1 } }`, quest 2002's.
        assert_eq!(
            Partikel::of(&[""], &[3940, 1706, 44287, 1]),
            Some(Partikel {
                at: None,
                cell: Some((3940, 1706)),
                colour: 0xFF00_ACFF,
                variant: 1,
            })
        );
        // A creature's, of no cell; the colour −1 white.
        let at = Partikel::of(&["Res:17246"], &[-1, -1, -1]).unwrap();
        assert_eq!(
            (at.at.as_deref(), at.cell, at.colour, at.variant),
            (Some("res:17246"), None, 0xFFFF_FFFF, 0)
        );
        // The statues' two numbers are not followed.
        assert_eq!(Partikel::of(&[], &[3, 60]), None);
    }
}
