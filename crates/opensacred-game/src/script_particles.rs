//! The scripts' `Partikel`s (99, `0x4A8BB0`; #1828; docs/re/netscript.md, "`Partikel`"): each
//! makes the generic system 856 and starts it with the FX event `0x50` in the record's colour and
//! of its variant, at the creature that its first name names (`hero` the hero), else at the centre
//! of its cell on the ground, else nowhere. The game makes the variants 1 and 4, a ring of sparks
//! rising about the place, the variant 4 with `SOUND_FX_SPAWN` (`opensacred_rules::particles::
//! generic_ring`), drawn as the other bursts (`puffs`); at a creature the ring stands where it
//! stood, as the original places the system once at the creature's root.
//!
//! The statues' power-up (`statue_ok`, `Partikel`'s two numbers and no name, `0x4A9502`; #1846)
//! gives the hero the effect 33 of the whole value of the first number for the seconds of the
//! second (`0x582D50(33, n0, 0, n1, …)`), whose adder takes its value of 0 for a half, heals
//! the hero whole and readies its arts (`0x5841DA`), plays its pick-up's clip (172) where it
//! stands, its gold sparks of the variant 2 on its body (`particles::statue_sparks`), and the
//! jingle `LEVELUP01`. Seen in the original (#1846): the power-up 2 raised the hero's third
//! damage factor from 1 to 1.5.

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
    heroes: Query<(Entity, &Transform), With<Body>>,
    (children, names, places): (Query<&Children>, Query<&Name>, Query<&GlobalTransform>),
    creatures: Query<(&ScriptName, &Transform), Without<Body>>,
    mut sounds: MessageWriter<Play>,
) {
    for partikel in made.read() {
        let hero = heroes.iter().next().map(|(_, at)| at.translation);
        if let (Some((whole, seconds)), Some((entity, at))) =
            (partikel.power_up, heroes.iter().next())
        {
            power_up(&mut commands, entity, (whole, seconds), &mut sounds);
            let at = at.translation;
            let points = crate::undead::bones(entity, at, (&children, &names, &places));
            let burst = particles::statue_sparks(&points, partikel.colour, &mut dice.0);
            crate::puffs::spawn(&mut commands, burst, at, None);
            continue;
        }
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

/// The jingle of a statue's power-up, the level-up's (`LEVELUP01`).
const POWER_UP_SOUND: u32 = 7002;
/// The effect that a statue's power-up gives the hero (`0x4A954A`).
const POWER_UP: u32 = 33;

/// A statue's power-up on the hero `hero` of the whole value `whole` for `seconds` seconds: its
/// effect 33 of the adder's half (whose whole values 0 to 3 add it to a damage kind's factor,
/// 4 and 5 the potions' bits, docs/re/inventory.md), its health whole, its arts ready (the
/// adder's `0x562470` of −1000 s for both kinds), its pick-up's clip and the jingle; the
/// adder's effect 30 of a second is not followed.
fn power_up(
    commands: &mut Commands,
    hero: Entity,
    (whole, seconds): (i32, i32),
    sounds: &mut MessageWriter<Play>,
) {
    let effect = opensacred_rules::fight::Effect {
        whole,
        ..opensacred_rules::fight::Effect::new(
            POWER_UP,
            opensacred_rules::fight::effects::DAMAGE_OF_GIVEN,
        )
    };
    let seconds = f32::from(i16::try_from(seconds).unwrap_or(i16::MAX));
    commands
        .entity(hero)
        .insert(crate::pickup::Kneels)
        .entry::<crate::effects::Effects>()
        .or_default()
        .and_modify(move |mut effects| effects.add_alone(effect, seconds));
    commands.queue(move |world: &mut World| {
        if let Some(mut health) = world.get_mut::<crate::fight::Health>(hero) {
            health.current = health.most;
        }
        if let Some(mut arts) = world.get_resource_mut::<crate::arts::HeroArts>() {
            arts.ready();
        }
    });
    sounds.write(Play {
        sound: Of::Id(POWER_UP_SOUND),
        at: None,
        rate: (0.5, Roll::None),
    });
    debug!("script_particles: the hero's power-up {whole} for {seconds} s");
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
            power_up: None,
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
                power_up: None,
            })
        );
        // A creature's, of no cell; the colour −1 white.
        let at = Partikel::of(&["Res:17246"], &[-1, -1, -1]).unwrap();
        assert_eq!(
            (at.at.as_deref(), at.cell, at.colour, at.variant),
            (Some("res:17246"), None, 0xFFFF_FFFF, 0)
        );
        // The statues' two numbers and no name: the power-up on the hero.
        let statue = Partikel::of(&[], &[3, 60]).unwrap();
        assert_eq!(
            (statue.at.as_deref(), statue.variant, statue.power_up),
            (Some("hero"), 2, Some((3, 60)))
        );
        // Two numbers and a name do nothing.
        assert_eq!(Partikel::of(&["res:1"], &[3, 60]), None);
    }
}
