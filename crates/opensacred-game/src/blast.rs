//! The Daemon's blast as his charge 1091 lands, the object 1168 (`FX_KAMIKAZE`, #760;
//! docs/re/arts.md, "The objects 1168 and 842"): about where he landed, for 3 s, its radius grows
//! with its time to three quarters of its most and then falls (`opensacred_rules::arts::Blast`).
//! At each frame each hostile creature alive on its level nearer it than its size and the radius,
//! once, takes the Daemon's strike of the charge's art and the share by his distance from it,
//! while he lives (`Blast::share`; seen, #856), and then, by the kicks' roll of the charge's level (**hypothesis**: the
//! object's level word, which the original leaves as it finds it), is knocked back by the blast's
//! strength (`making::Knocked`) (docs/decisions/arts.md, "Daemon's blast edge and level"). It
//! looks as its system, the ring of poison's with its own reach and colours (`poison_rings`,
//! #1092).

use bevy::prelude::*;
use opensacred_rules::arts::{BLAST_LIFE, Blast};

use crate::ai::Mind;
use crate::crowd::InTheWay;
use crate::fight::{self, Health};
use crate::frame::{Playing, Step};
use crate::hero::Body;
use crate::npcs::Dies;
use crate::spawning::Rng;
use crate::terrain::Level;

/// A blast: its caster, its values, its level, the level of the charge that made it, its place,
/// its time, its radius, and the creatures it has touched.
#[derive(Component)]
pub struct Blasting {
    caster: Entity,
    blast: Blast,
    level: Level,
    art_level: u32,
    at: Vec3,
    time: f32,
    radius: f32,
    touched: Vec<Entity>,
}

impl Blasting {
    /// The blast `blast` of `caster`'s charge of the level in play `art_level` at `at`, on
    /// `level`.
    pub fn new(caster: Entity, blast: Blast, level: Level, art_level: u32, at: Vec3) -> Self {
        Self {
            caster,
            blast,
            level,
            art_level,
            at,
            time: 0.0,
            radius: 0.0,
            touched: Vec::new(),
        }
    }
}

/// The dice of the knockbacks.
#[derive(Resource)]
struct Dice(Rng);

/// Gives each new blast its look (`0x7C1AB0`): the ring of its reach, `R` a second
/// (`poison_rings`).
fn show(
    blasts: Query<&Blasting, Added<Blasting>>,
    mut rings: MessageWriter<crate::poison_rings::Made>,
) {
    for blasting in &blasts {
        rings.write(crate::poison_rings::Made {
            at: blasting.at,
            kind: crate::poison_rings::Kind::Blast(blasting.blast.most),
        });
    }
}

/// Spreads each blast a frame on and strikes what it reaches (`0x5A6680`, `0x5AB490`,
/// `0x5ABD80` at `0x5B1DFB`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn spread(
    mut commands: Commands,
    time: Res<Time>,
    mut dice: ResMut<Dice>,
    mut blasts: Query<(Entity, &mut Blasting)>,
    casters: Query<&Transform, (With<Body>, Without<Blasting>, Without<Dies>)>,
    creatures: Query<
        (
            Entity,
            &Transform,
            &InTheWay,
            &Mind,
            &Health,
            Option<&crate::arrows::Size>,
        ),
        (Without<Body>, Without<Blasting>, Without<Dies>),
    >,
    mut hits: MessageWriter<fight::Hit>,
) {
    for (entity, mut blasting) in &mut blasts {
        blasting.time += time.delta_secs();
        if blasting.time >= BLAST_LIFE {
            commands.entity(entity).despawn();
            continue;
        }
        let radius = blasting.blast.radius(blasting.time, blasting.radius);
        blasting.radius = radius;
        let at = blasting.at.xz();
        let caster = casters.get(blasting.caster).ok().map(|t| t.translation);
        let reached: Vec<(Entity, Vec3, &Mind)> = creatures
            .iter()
            .filter(|(e, place, way, mind, health, size)| {
                way.creature
                    && way.level == blasting.level
                    && mind.hostile()
                    && health.current > 0
                    && !blasting.touched.contains(e)
                    && (place.translation.xz() - at).length()
                        < size.map_or(crate::arrows::size(0), |size| size.0) + radius
            })
            .map(|(e, place, _, mind, ..)| (e, place.translation, mind))
            .collect();
        for (target, place, mind) in reached {
            blasting.touched.push(target);
            let Some(from) = caster else {
                continue;
            };
            let distance = crate::making::whole_distance(from, place);
            if let Some(share) = blasting.blast.share(distance) {
                debug!(
                    "blast: {entity} of {} strikes {target} {distance} off, its share {share:.3}",
                    blasting.caster
                );
                hits.write(fight::Hit {
                    attacker: blasting.caster,
                    target,
                    share,
                    ranged: false,
                    art: Some(blasting.art_level),
                    fixed: None,
                    sure: false,
                    poison: false,
                });
            }
            if crate::making::knocks_back(&mut dice.0, blasting.art_level, mind) {
                commands
                    .entity(target)
                    .remove::<(crate::npcs::Walk, crate::npcs::Strikes)>()
                    .insert(crate::making::Knocked {
                        by: blasting.caster,
                        from,
                        left: blasting.blast.strength,
                        held: 0.0,
                        throw: None,
                    });
            }
        }
    }
}

/// The Daemon's blasts.
pub struct BlastPlugin;

impl Plugin for BlastPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Dice(crate::seed::dice(5))).add_systems(
            Update,
            (show, spread).chain().after(Step::Npcs).in_set(Playing),
        );
    }
}
