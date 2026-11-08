//! The combat arts' hit sparks (#1092's fifth family; docs/re/effects.md, "The hit sparks"): every
//! strike of a hero's art that sparks (`opensacred_rules::particles::sparks_of`), whether it lands
//! or not, bursts its class's sparks once at the struck creature's neck, else its root, or for a
//! kick at the kicker's foot (the system flag `0x40`): `FX_GLAD_SM_HIT` (842) green for the
//! Gladiator, the Dwarf, the Daemon and the Vampiress's kick, `FX_SERA_SM_HIT` (843) blue for the
//! Seraphim and the Mage's `Hardhit` and `Rundum`, `FX_ELVE_SM_HIT` (860) green for the Wood Elf
//! and the Mage's `Attacke`, `FX_VAMP_SM_HIT` (862) red for the Vampiress, `FX_DARK_SM_HIT` (864)
//! yellow for the Dark Elf (`particles::hit_sparks`, drawn by `puffs`, untinted by the night); and
//! its object lights the night by 200 at the striker's place on the ground while it lives.
//!
//! The Dwarf's `Attacke` and `Hardhit` with a gun make none (`0x514DC4` to `0x515011`). Not
//! followed: the variants of the sparks' classes that are not hits (the Gladiator's rings of 1015
//! and 1016, the Seraphim's 1003, the Vampiress's 1030, 1031 and 1033, the Dark Elf's 1042, 1046 to
//! 1048, a horse's floops, #2329), and the sparks of the thrown weapons' and the Wood Elf's arrows'
//! hits. A horse's trample sends the awe's at each creature it knocks and at the horse (`trample`).

use bevy::prelude::*;
use opensacred_rules::arts::{DWARF_ATTACKE, DWARF_HARDHIT};
use opensacred_rules::particles::{SPARKS_LIFE, SPARKS_LIGHT, SparksAt, hit_sparks, sparks_of};
use opensacred_rules::spawning::Rng;

use crate::frame::Playing;
use crate::game::AppState;

/// A strike of the hero's art `art` at the creature `on`, which `making` sends with each strike.
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

/// The sparks' rolls.
#[derive(Resource)]
struct Rolls(Rng);

impl Default for Rolls {
    fn default() -> Self {
        Self(crate::seed::dice(0x843))
    }
}

/// Bursts each strike's sparks where its art makes them, and lights its striker's place.
fn burst(
    mut commands: Commands,
    mut sparks: MessageReader<Spark>,
    hero: Option<Res<crate::hero::Hero>>,
    places: Query<&GlobalTransform>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    mut rolls: ResMut<Rolls>,
) {
    let gun = hero.is_some_and(|hero| hero.armed.gun);
    for spark in sparks.read() {
        let Some((kind, at)) = sparks_of(spark.art) else {
            continue;
        };
        if gun && matches!(spark.art, DWARF_ATTACKE | DWARF_HARDHIT) {
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
            .add_systems(
                Update,
                (burst, put_out)
                    .in_set(Playing)
                    .after(crate::frame::Step::Npcs)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
