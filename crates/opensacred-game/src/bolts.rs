//! The Seraphim's bolts (#754; docs/re/arts.md, "Her shots with 1018's and 1019's effects" and
//! "The Seraphim's bolts"): while her effect 17 holds each of her shots makes two railgun bolts,
//! the second [`RAILGUN_SECOND`] after the first, and while her effect 16 holds a shot beyond 200
//! makes one BFG bolt (`attack`). A bolt leaves where she stands along the line to her target, at
//! a hand's height over the ground, flies straight at its speed for its time at most and ends at a
//! cell that may not be walked on; in each frame it finds the creatures on its level, alive and
//! hostile to the hero, nearer it than the sum of their sizes, and hits each with its damages (the
//! fight's hit of fixed damages, `opensacred_rules::arts::bolt_damages`): the railgun's hits all
//! it found once and goes, the BFG's hits those of its first contact and flies on harmless. A
//! glowing ball stands in for their particle systems (docs/decisions/2026-10-24-bolts-look.md).

use bevy::prelude::*;
use opensacred_rules::arts::{Bolt, RAILGUN_SECOND, bolt_damages};

use crate::crowd::InTheWay;
use crate::effects::Effects;
use crate::fight::{self, Combat, Health};
use crate::frame::{Playing, Step};
use crate::game::AppState;
use crate::hero::Body;
use crate::npcs::Dies;
use crate::terrain::{Heights, Level};
use crate::{ai, arrows, paths};

/// How high over the ground a bolt flies, in world units (**hypothesis**: about a hand's height,
/// as the arrows leave a shooter without a hand).
const HEIGHT: f32 = 80.0;

/// A shot's bolt leaving its shooter toward `toward` (the FX events of `0x4DCB80`), which the
/// shot's clip sends at its first hit (`attack`).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Fire {
    pub shooter: Entity,
    pub toward: Vec3,
    pub bolt: Bolt,
}

/// A bolt in flight: its kind, its shooter, its direction on the ground, the level it flies on,
/// its damages, how long until it leaves and how long it flies on, and whether it hit already (a
/// BFG's, which flies on harmless).
#[derive(Component)]
struct Flying {
    bolt: Bolt,
    shooter: Entity,
    direction: Vec2,
    level: Level,
    damages: [f32; 4],
    wait: f32,
    left: f32,
    spent: bool,
}

/// The stand-ins' meshes and materials, the railgun's and the BFG's.
#[derive(Resource)]
struct Looks([(Handle<Mesh>, Handle<StandardMaterial>); 2]);

impl Looks {
    fn of(&self, bolt: Bolt) -> (Handle<Mesh>, Handle<StandardMaterial>) {
        self.0[usize::from(bolt == Bolt::Bfg)].clone()
    }
}

/// Makes the stand-ins' looks: a small blue-white ball for the railgun, a larger green one for
/// the BFG (**estimate**: the colours of their glows).
fn make_looks(
    mut commands: Commands,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };
    let mut look = |radius: f32, colour: Color| {
        (
            meshes.add(Sphere::new(radius)),
            materials.add(StandardMaterial {
                base_color: colour,
                unlit: true,
                ..default()
            }),
        )
    };
    commands.insert_resource(Looks([
        look(10.0, Color::srgb(0.75, 0.85, 1.0)),
        look(22.0, Color::srgb(0.55, 1.0, 0.45)),
    ]));
}

/// Makes the bolts of each shot that leaves: where the shooter stands, at [`HEIGHT`], along the
/// line on the ground to `toward` (with none, its facing), on the shooter's level, of the damages
/// of the bolt's record by the whole value of its effect and the shooter's multipliers.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn fire(
    mut commands: Commands,
    mut shots: MessageReader<Fire>,
    looks: Option<Res<Looks>>,
    arts: Option<Res<crate::arts::HeroArts>>,
    shooters: Query<(&Transform, &Body, Option<&Combat>, Option<&Effects>), Without<Flying>>,
) {
    let (Some(looks), Some(arts)) = (looks, arts) else {
        shots.clear();
        return;
    };
    for shot in shots.read() {
        let Ok((at, body, combat, effects)) = shooters.get(shot.shooter) else {
            continue;
        };
        let level = effects
            .and_then(|effects| {
                effects
                    .list()
                    .into_iter()
                    .find(|effect| effect.id == shot.bolt.effect())
            })
            .map_or(1, |effect| effect.whole);
        let Some(spell) = arts
            .tables
            .spells
            .iter()
            .find(|spell| spell.id == shot.bolt.spell())
        else {
            warn!("bolts: no record {} of the first table", shot.bolt.spell());
            continue;
        };
        let multipliers = combat.map_or([1.0; 4], |combat| combat.multipliers);
        let damages = bolt_damages(spell, level, multipliers);
        let here = at.translation;
        let direction = (shot.toward - here)
            .xz()
            .try_normalize()
            .unwrap_or_else(|| {
                let facing = at.rotation * Vec3::Z;
                facing.xz().normalize_or(Vec2::Y)
            });
        let (_, life) = shot.bolt.flight();
        let (mesh, material) = looks.of(shot.bolt);
        debug!(
            "bolts: {} fires {} {:?} of {damages:?} from {:.0},{:.0} along {:.2},{:.2}",
            shot.shooter,
            shot.bolt.count(),
            shot.bolt,
            here.x,
            here.z,
            direction.x,
            direction.y
        );
        for i in 0..shot.bolt.count() {
            let wait = if i == 0 { 0.0 } else { RAILGUN_SECOND };
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(here + Vec3::Y * HEIGHT),
                if wait > 0.0 {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                },
                Flying {
                    bolt: shot.bolt,
                    shooter: shot.shooter,
                    direction,
                    level: body.level(),
                    damages,
                    wait,
                    left: life,
                    spent: false,
                },
                DespawnOnExit(AppState::InGame),
            ));
        }
    }
}

/// Flies each bolt a frame on (`0x5A6680`) and hits what it comes on (`0x5AB490`, `0x5ABD80`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    heights: Res<Heights>,
    mut hits: MessageWriter<fight::Hit>,
    mut flying: Query<(Entity, &mut Flying, &mut Transform, &mut Visibility)>,
    shooters: Query<&Transform, (With<Body>, Without<Flying>)>,
    creatures: Query<
        (
            Entity,
            &Transform,
            &InTheWay,
            &ai::Mind,
            &Health,
            Option<&arrows::Size>,
        ),
        (Without<Body>, Without<Flying>, Without<Dies>),
    >,
) {
    let dt = time.delta_secs();
    for (entity, mut bolt, mut transform, mut visibility) in &mut flying {
        // The second railgun bolt leaves its shooter's place as its time comes.
        if bolt.wait > 0.0 {
            bolt.wait -= dt;
            if let Ok(shooter) = shooters.get(bolt.shooter) {
                transform.translation = shooter.translation + Vec3::Y * HEIGHT;
            }
            if bolt.wait <= 0.0 {
                *visibility = Visibility::Inherited;
            }
            continue;
        }
        bolt.left -= dt;
        let (speed, _) = bolt.bolt.flight();
        let step = bolt.direction * speed * dt;
        let ground = transform.translation.xz() + step;
        let cell = paths::cell_of(ground.x, ground.y);
        if bolt.left <= 0.0 || !heights.walkable_on(cell, bolt.level) {
            debug!(
                "bolts: {entity} of {} ends at {:.0},{:.0}",
                bolt.shooter, ground.x, ground.y
            );
            commands.entity(entity).despawn();
            continue;
        }
        let height = heights
            .standing(ground.x, ground.y, bolt.level)
            .unwrap_or(transform.translation.y - HEIGHT);
        transform.translation = Vec3::new(ground.x, height + HEIGHT, ground.y);
        if bolt.spent {
            continue;
        }
        // The creatures on its level, alive and hostile to the hero, nearer than the two sizes.
        let own = arrows::size(0);
        let struck: Vec<Entity> = creatures
            .iter()
            .filter(|(_, at, way, mind, health, size)| {
                way.creature
                    && way.level == bolt.level
                    && mind.hostile()
                    && health.current > 0
                    && (at.translation.xz() - ground).length()
                        < own + size.map_or(arrows::size(0), |size| size.0)
            })
            .map(|(e, ..)| e)
            .collect();
        if struck.is_empty() {
            continue;
        }
        for &target in &struck {
            debug!(
                "bolts: {entity} of {} hits {target} for {:?}",
                bolt.shooter, bolt.damages
            );
            hits.write(fight::Hit {
                attacker: bolt.shooter,
                target,
                share: 1.0,
                ranged: false,
                art: None,
                fixed: Some(bolt.damages),
            });
        }
        if bolt.bolt.goes() {
            commands.entity(entity).despawn();
        } else {
            bolt.spent = true;
        }
    }
}

/// The Seraphim's bolts.
pub struct BoltsPlugin;

impl Plugin for BoltsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Fire>()
            .add_systems(Startup, make_looks)
            .add_systems(
                Update,
                (fire, fly).chain().after(Step::Npcs).in_set(Playing),
            );
    }
}
