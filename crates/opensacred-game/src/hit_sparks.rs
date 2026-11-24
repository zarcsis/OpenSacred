//! The combat arts' hit sparks (#1092's fifth family; docs/re/effects.md, "The hit sparks"): every
//! strike of a hero's art that sparks (`opensacred_rules::particles::sparks_of`), whether it lands
//! or not, bursts its class's sparks once at the struck creature's neck, else its root, or for a
//! kick at the kicker's foot (the system flag `0x40`): `FX_GLAD_SM_HIT` (842) green for the
//! Gladiator, the Dwarf, the Daemon and the Vampiress's kick, `FX_SERA_SM_HIT` (843) blue for the
//! Seraphim and the Mage's `Hardhit` and `Rundum`, `FX_ELVE_SM_HIT` (860) green for the Wood Elf
//! and the Mage's `Attacke`, `FX_VAMP_SM_HIT` (862) red for the Vampiress, `FX_DARK_SM_HIT` (864)
//! yellow for the Dark Elf (`particles::hit_sparks`, drawn by `puffs`, untinted by the night); and
//! its object lights the night by 200 at the striker's place on the ground while it lives. A
//! hero's flight that a cell stops bursts the Dark Elf's at the hero's own neck, whatever the class
//! (`making::leap`, #3203). The Gladiator's hurled weapon 1013 bursts the Dark Elf's at the
//! creature it strikes (`hurled`, `0x44E570`), and the Wood Elf's explosive arrow 1004 the Wood
//! Elf's at a creature its strike leaves alive, once the strike is resolved (`arrows`,
//! `opensacred_rules::particles::sparks_if_alive`, `0x5BC958`), its light at that creature, the
//! object's caster (#3129); a creature that its strike leaves dead takes them too where it rises
//! again, an undead whose death is not its last, and else bursts (`undead::Bursts`, `0x4E8DA0(1,
//! 2)`: its cloud, `CHAR_SPLASH`, its body gone, and under the violence its parts and its blood;
//! `0x5BC8FE`–`0x5BC9E9`, #3130).
//!
//! The Dwarf's `Attacke`, `Hardhit` and `Rundum` with a gun make none (`0x514DC4` to `0x515011`,
//! `0x515C21` to `0x516028`). Not
//! followed: the variants of the sparks' classes that are not hits (the Gladiator's rings of 1015
//! and 1016, the Seraphim's 1003); the club arrow's ring and the spider arrow's smoke are
//! `arrow_sights`'; a horse's floops are `floops`' (#2329), the Dark Elf's 1042 and 1046 to 1048
//! `dark_sights`' (#2923), the Vampiress's 1030, 1031 and 1033 `bite_sights`' (#3062). A horse's trample sends the awe's at each creature it knocks and at the
//! horse (`trample`), and a riding creature's strike in passing the same at its target (`ai`).

use bevy::prelude::*;
use opensacred_rules::arts::{DWARF_ATTACKE, DWARF_HARDHIT, DWARF_RUNDUM};
use opensacred_rules::particles::{
    SPARKS_LIFE, SPARKS_LIGHT, SparksAt, hit_sparks, sparks_if_alive, sparks_of,
};
use opensacred_rules::spawning::Rng;

use crate::frame::Playing;
use crate::game::AppState;

/// A strike of the art `art` by `by` at the creature `on`: `making` sends one with each strike of
/// a hero's art, `trample` and `ai` the awe's for a horse's trample and a rider's passing strike,
/// `hurled` the hurled weapon's and `arrows` the explosive arrow's.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spark {
    pub art: u16,
    pub by: Entity,
    pub on: Entity,
}

/// The neck's and the feet's bones (3, 36 and 37).
const NECK: [&str; 1] = ["Bip01 Neck"];
const RIGHT_FOOT: [&str; 1] = ["Bip01 R Foot"];
const LEFT_FOOT: [&str; 1] = ["Bip01 L Foot"];

/// A sparks' light at its striker's place for as long as the sparks live, the seconds left.
#[derive(Component)]
struct Lamp(f32);

/// The strikes whose sparks wait for their strike's resolution, to burst where it leaves its
/// creature alive ([`sparks_if_alive`]).
#[derive(Resource, Default)]
struct Waiting(Vec<Waits>);

/// A strike's sparks that wait: the frames they have waited, and whether the strike left its
/// creature dead, whose death tells whether it rises again.
#[derive(Clone, Copy, Debug)]
struct Waits {
    spark: Spark,
    frames: u8,
    dead: bool,
}

/// How many frames a strike's sparks wait for its resolution at most: `fight`'s resolution reads
/// a strike in its frame or the next.
const WAITS: u8 = 3;

/// The sparks' rolls.
#[derive(Resource)]
struct Rolls(Rng);

impl Default for Rolls {
    fn default() -> Self {
        Self(crate::seed::dice(0x843))
    }
}

/// Bursts each strike's sparks where its art makes them, and lights its striker's place; those of
/// an art that sparks only at a creature left alive wait for their strike's resolution
/// (`fight::Struck`) and burst where its creature lives on, or where its death is not its last
/// (`npcs::Dies::rises`); a creature that dies of it otherwise bursts ([`crate::undead::Bursts`],
/// `0x5BC8FE`–`0x5BC9E9`).
fn burst(
    mut commands: Commands,
    (mut sparks, mut struck): (MessageReader<Spark>, MessageReader<crate::fight::Struck>),
    hero: Option<Res<crate::hero::Hero>>,
    (places, lives, deaths): (
        Query<&GlobalTransform>,
        Query<&crate::fight::Health, Without<crate::npcs::Dies>>,
        Query<&crate::npcs::Dies>,
    ),
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut rolls, mut waiting): (ResMut<Rolls>, ResMut<Waiting>),
) {
    let gun = hero.is_some_and(|hero| hero.armed.gun);
    let mut due: Vec<Spark> = Vec::new();
    for &spark in sparks.read() {
        if sparks_if_alive(spark.art) {
            waiting.0.push(Waits {
                spark,
                frames: 0,
                dead: false,
            });
        } else {
            due.push(spark);
        }
    }
    let resolved: Vec<Entity> = struck.read().map(|struck| struck.target).collect();
    waiting.0.retain_mut(|waits| {
        let on = waits.spark.on;
        if !waits.dead && resolved.contains(&on) {
            if lives.get(on).is_ok_and(|health| health.current > 0) {
                due.push(waits.spark);
                return false;
            }
            waits.dead = true;
            waits.frames = 0;
        }
        // Dead of it: the sight where it rises again, else its burst (`0x549460(1)`).
        if waits.dead
            && let Ok(dies) = deaths.get(on)
        {
            if dies.rises {
                due.push(waits.spark);
            } else {
                debug!("hit_sparks: {on}, dead of {}, bursts", waits.spark.art);
                commands.entity(on).insert(crate::undead::Bursts);
            }
            return false;
        }
        waits.frames += 1;
        waits.frames <= WAITS
    });
    for spark in &due {
        let Some((kind, at)) = sparks_of(spark.art) else {
            continue;
        };
        if gun && matches!(spark.art, DWARF_ATTACKE | DWARF_HARDHIT | DWARF_RUNDUM) {
            continue;
        }
        let (whom, names): (Entity, &[&str]) = match at {
            SparksAt::Neck => (spark.on, &NECK),
            SparksAt::Root => (spark.on, &[]),
            SparksAt::RightFoot => (spark.by, &RIGHT_FOOT),
            SparksAt::LeftFoot => (spark.by, &LEFT_FOOT),
        };
        let Some(point) = crate::making::bone(whom, names, &children, (&bones, &worn))
            .or_else(|| places.get(whom).ok().map(GlobalTransform::translation))
        else {
            continue;
        };
        debug!("hit_sparks: {:?} of {} at {point}", kind, spark.art);
        crate::puffs::spawn(&mut commands, hit_sparks(kind, &mut rolls.0), point, None);
        if let Ok(by) = places.get(spark.by) {
            commands.spawn((
                Lamp(SPARKS_LIFE),
                crate::daynight::Light(SPARKS_LIGHT),
                Transform::from_translation(by.translation().with_y(0.0)),
                DespawnOnExit(AppState::InGame),
            ));
        }
    }
}

/// Puts out each sparks' light as its sparks die.
fn put_out(mut commands: Commands, time: Res<Time>, mut lamps: Query<(Entity, &mut Lamp)>) {
    for (entity, mut lamp) in &mut lamps {
        lamp.0 -= time.delta_secs();
        if lamp.0 <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// The hit sparks.
pub struct HitSparksPlugin;

impl Plugin for HitSparksPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Spark>()
            .init_resource::<Rolls>()
            .init_resource::<Waiting>()
            // After the strikes' resolution, whose deaths it reads, and before the dead are laid
            // down as corpses (`npcs::die`), which a burst takes at once (`undead`).
            .add_systems(
                Update,
                (burst, put_out)
                    .in_set(Playing)
                    .after(crate::fight::resolve)
                    .before(crate::frame::Step::Npcs)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fight::{Health, Struck};

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<Spark>()
            .add_message::<Struck>()
            .init_resource::<Rolls>()
            .init_resource::<Waiting>()
            .add_systems(Update, burst);
        app
    }

    fn creature(app: &mut App, current: u32) -> Entity {
        app.world_mut()
            .spawn((
                Transform::default(),
                GlobalTransform::default(),
                Health {
                    current,
                    most: 50,
                    level: 1,
                },
            ))
            .id()
    }

    fn bursts(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<(), With<crate::puffs::Unmade>>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn the_explosive_arrows_sparks_wait_for_its_strike_and_burst_where_it_lives() {
        let mut app = app();
        let alive = creature(&mut app, 20);
        let dead = creature(&mut app, 0);
        for on in [alive, dead] {
            app.world_mut().write_message(Spark {
                art: 1004,
                by: on,
                on,
            });
        }
        app.update();
        // Not before its strike is resolved.
        assert_eq!(bursts(&mut app), 0);
        for target in [alive, dead] {
            app.world_mut().write_message(Struck {
                target,
                damage: 30,
                most: 50,
            });
        }
        app.update();
        // At the creature left alive alone; the dead one waits for its death.
        assert_eq!(bursts(&mut app), 1);
        assert_eq!(app.world().resource::<Waiting>().0.len(), 1);
    }

    #[test]
    fn a_creature_the_explosive_arrow_kills_bursts_but_where_it_rises_again() {
        let mut app = app();
        let dead = creature(&mut app, 0);
        let rises = creature(&mut app, 0);
        app.world_mut()
            .entity_mut(dead)
            .insert(crate::npcs::Dies::new(0));
        let mut rising = crate::npcs::Dies::new(0);
        rising.rises = true;
        app.world_mut().entity_mut(rises).insert(rising);
        for on in [dead, rises] {
            app.world_mut().write_message(Spark {
                art: 1004,
                by: on,
                on,
            });
            app.world_mut().write_message(Struck {
                target: on,
                damage: 60,
                most: 50,
            });
        }
        app.update();
        // The dead bursts, its sparks none; the undead that will rise takes the sight.
        assert!(app.world().get::<crate::undead::Bursts>(dead).is_some());
        assert!(app.world().get::<crate::undead::Bursts>(rises).is_none());
        assert_eq!(bursts(&mut app), 1);
        assert!(app.world().resource::<Waiting>().0.is_empty());
    }

    #[test]
    fn a_hurled_weapons_sparks_burst_at_once_and_unresolved_ones_are_given_up() {
        let mut app = app();
        let on = creature(&mut app, 0);
        app.world_mut().write_message(Spark {
            art: 1013,
            by: on,
            on,
        });
        app.world_mut().write_message(Spark {
            art: 1004,
            by: on,
            on,
        });
        app.update();
        assert_eq!(bursts(&mut app), 1);
        for _ in 0..=WAITS {
            app.update();
        }
        assert!(app.world().resource::<Waiting>().0.is_empty());
    }
}
