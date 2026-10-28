//! The Daemon's blast as his charge 1091 lands, the object 1168 (`FX_KAMIKAZE`, #760;
//! docs/re/arts.md, "The objects 1168 and 842"): about where he landed, for 3 s, its radius grows
//! with its time to three quarters of its most and then falls (`opensacred_rules::arts::Blast`).
//! At each frame each hostile creature alive on its level nearer it than its size and the radius,
//! once, takes the Daemon's strike of the charge's art and the share by his distance from it,
//! while he lives (`Blast::share`; seen, #856), and then, by the kicks' roll of the charge's level (**hypothesis**: the
//! object's level word, which the original leaves as it finds it), is knocked back by the blast's
//! strength (`making::Knocked`). A ring of one colour stands in for its particle system
//! (docs/decisions/arts.md, "Daemon's blast edge and level").

use bevy::prelude::*;
use opensacred_rules::arts::{BLAST_LIFE, Blast};

use crate::ai::Mind;
use crate::crowd::InTheWay;
use crate::fight::{self, Health};
use crate::frame::{Playing, Step};
use crate::game::AppState;
use crate::hero::Body;
use crate::npcs::Dies;
use crate::spawning::Rng;
use crate::terrain::Level;

/// How high the ring stands, in world units.
const RING_HEIGHT: f32 = 20.0;

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

/// The ring's look.
#[derive(Resource)]
struct Look(Handle<Mesh>, Handle<StandardMaterial>);

/// The dice of the knockbacks.
#[derive(Resource)]
struct Dice(Rng);

/// Makes the ring's look: a flat cylinder of radius 1, scaled to the blast's radius, of a
/// translucent orange (**estimate**).
fn make_look(
    mut commands: Commands,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };
    commands.insert_resource(Look(
        meshes.add(Cylinder::new(1.0, RING_HEIGHT)),
        materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.55, 0.15, 0.35),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
    ));
}

/// Gives each new blast its ring.
fn show(
    mut commands: Commands,
    look: Option<Res<Look>>,
    blasts: Query<(Entity, &Blasting), Added<Blasting>>,
) {
    let Some(look) = look else {
        return;
    };
    for (entity, blasting) in &blasts {
        commands.entity(entity).insert((
            Mesh3d(look.0.clone()),
            MeshMaterial3d(look.1.clone()),
            Transform::from_translation(blasting.at + Vec3::Y * RING_HEIGHT / 2.0)
                .with_scale(Vec3::new(0.0, 1.0, 0.0)),
            DespawnOnExit(AppState::InGame),
        ));
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
    mut blasts: Query<(Entity, &mut Blasting, Option<&mut Transform>)>,
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
    for (entity, mut blasting, transform) in &mut blasts {
        blasting.time += time.delta_secs();
        if blasting.time >= BLAST_LIFE {
            commands.entity(entity).despawn();
            continue;
        }
        let radius = blasting.blast.radius(blasting.time, blasting.radius);
        blasting.radius = radius;
        if let Some(mut transform) = transform {
            transform.scale = Vec3::new(radius, 1.0, radius);
        }
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
        app.insert_resource(Dice(Rng::of(0, 0, 5)))
            .add_systems(Startup, make_look)
            .add_systems(
                Update,
                (show, spread).chain().after(Step::Npcs).in_set(Playing),
            );
    }
}
