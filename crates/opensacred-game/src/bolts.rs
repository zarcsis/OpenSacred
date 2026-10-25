//! The bolts: the Seraphim's (#754; docs/re/arts.md, "Her shots with 1018's and 1019's effects"
//! and "The Seraphim's bolts") and the Daemon's 1103's (#760, "The Daemon's"). While her effect 17
//! holds each of her shots makes two railgun bolts, the second [`RAILGUN_SECOND`] after the first,
//! and while her effect 16 holds a shot beyond 200 makes one BFG bolt (`attack`); his 1103 makes a
//! fan of them (`making`). A bolt leaves where its shooter stands along its line, at a hand's
//! height over the ground, flies straight at its speed, which may slow, for its time at most and
//! ends at a cell that may not be walked on; in each frame it finds the creatures on its level,
//! alive and hostile to the hero, nearer it than the sum of their sizes, and hits them with its
//! damages (the fight's hit of fixed damages, which always lands, #854): the railgun's every one it
//! found and goes, the BFG's those of its first contact and flies on harmless, the Daemon's the
//! first alone and goes. The Mage's fireball flies as a bolt too, of its own speed and time by its
//! level, and strikes every creature it found at its first contact and goes (`making`, #872). A
//! glowing ball stands in for their particle systems (docs/decisions/2026-10-24-bolts-look.md).

use bevy::prelude::*;
use opensacred_rules::arts::{Bolt, EMPEROR_COURSE, RAILGUN_SECOND, bolt_damages};

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

/// A shot's bolts leaving their shooter toward `toward` (the FX events of `0x4DCB80`), which the
/// shot's clip sends at its first hit (`attack`).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Fire {
    pub shooter: Entity,
    pub toward: Vec3,
    pub bolt: Bolt,
}

/// What a bolt strikes as it meets creatures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strikes {
    /// Every creature it finds in a frame, once, and it goes (the railgun's).
    All,
    /// Those of its first contact, once, and it flies on harmless (the BFG's).
    FirstContact,
    /// The first creature it finds alone, and it goes (the Daemon's 1103's).
    First,
}

/// How a bolt flies and strikes, and its look: its speed in units a second, how much slower it
/// grows each second, its most time, what it strikes and its look's index.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Course {
    pub speed: f32,
    pub slows: f32,
    pub life: f32,
    pub strikes: Strikes,
    pub look: usize,
}

impl Course {
    /// The course of the Seraphim's bolt `bolt`.
    pub fn of(bolt: Bolt) -> Self {
        let (speed, life) = bolt.flight();
        let (strikes, look) = if bolt.goes() {
            (Strikes::All, 0)
        } else {
            (Strikes::FirstContact, 1)
        };
        Self {
            speed,
            slows: 0.0,
            life,
            strikes,
            look,
        }
    }

    /// The course of the Mage's fireball at the level `level` (the object 778, `0x76BE80`): every
    /// creature of its first contact, once, and it goes (`0x5ABD80` at `0x5AC015`).
    pub fn fireball(level: u32) -> Self {
        Self {
            speed: opensacred_rules::spells::fireball_speed(level),
            slows: 0.0,
            life: opensacred_rules::spells::fireball_life(level),
            strikes: Strikes::All,
            look: 3,
        }
    }

    /// The course of a bolt of the Daemon's 1103 (`cParticleSystem_emperor`).
    pub fn emperor() -> Self {
        let (speed, slows, life) = EMPEROR_COURSE;
        Self {
            speed,
            slows,
            life,
            strikes: Strikes::First,
            look: 2,
        }
    }
}

/// A bolt leaving its shooter along `direction` on the ground, of its course and its damages,
/// after `wait` seconds.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Launch {
    pub shooter: Entity,
    pub direction: Vec2,
    pub course: Course,
    pub damages: [f32; 4],
    pub wait: f32,
}

/// A bolt in flight: its course, its shooter, its direction on the ground, the level it flies on,
/// its damages, how long until it leaves, how long it has flown, and whether it hit already (a
/// BFG's, which flies on harmless).
#[derive(Component)]
struct Flying {
    course: Course,
    shooter: Entity,
    direction: Vec2,
    level: Level,
    damages: [f32; 4],
    wait: f32,
    flown: f32,
    spent: bool,
}

/// The stand-ins' meshes and materials: the railgun's, the BFG's, the Daemon's and the
/// fireball's.
#[derive(Resource)]
struct Looks(Vec<(Handle<Mesh>, Handle<StandardMaterial>)>);

/// Makes the stand-ins' looks: a small blue-white ball for the railgun, a larger green one for
/// the BFG, a violet one for the Daemon's, an orange one for the fireball (**estimate**: the
/// colours of their glows).
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
    commands.insert_resource(Looks(vec![
        look(10.0, Color::srgb(0.75, 0.85, 1.0)),
        look(22.0, Color::srgb(0.55, 1.0, 0.45)),
        look(14.0, Color::srgb(0.75, 0.45, 1.0)),
        look(18.0, Color::srgb(1.0, 0.55, 0.15)),
    ]));
}

/// Makes the launches of each of the Seraphim's shots: its bolts along the line on the ground to
/// `toward` (with none, her facing), of the damages of the bolt's record by the whole value of
/// its effect and her multipliers, the second railgun bolt [`RAILGUN_SECOND`] after the first.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn fire(
    mut shots: MessageReader<Fire>,
    mut launches: MessageWriter<Launch>,
    arts: Option<Res<crate::arts::HeroArts>>,
    shooters: Query<(&Transform, Option<&Combat>, Option<&Effects>), With<Body>>,
) {
    let Some(arts) = arts else {
        shots.clear();
        return;
    };
    for shot in shots.read() {
        let Ok((at, combat, effects)) = shooters.get(shot.shooter) else {
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
        let direction = (shot.toward - at.translation)
            .xz()
            .try_normalize()
            .unwrap_or_else(|| (at.rotation * Vec3::X).xz().normalize_or(Vec2::X));
        debug!(
            "bolts: {} fires {} {:?} of {damages:?}",
            shot.shooter,
            shot.bolt.count(),
            shot.bolt
        );
        for i in 0..shot.bolt.count() {
            launches.write(Launch {
                shooter: shot.shooter,
                direction,
                course: Course::of(shot.bolt),
                damages,
                wait: if i == 0 { 0.0 } else { RAILGUN_SECOND },
            });
        }
    }
}

/// Makes the bolt of each launch where its shooter stands, at [`HEIGHT`], on its level.
fn launch(
    mut commands: Commands,
    mut launches: MessageReader<Launch>,
    looks: Option<Res<Looks>>,
    shooters: Query<(&Transform, &Body)>,
) {
    let Some(looks) = looks else {
        launches.clear();
        return;
    };
    for launched in launches.read() {
        let Ok((at, body)) = shooters.get(launched.shooter) else {
            continue;
        };
        let Some((mesh, material)) = looks.0.get(launched.course.look).cloned() else {
            continue;
        };
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(at.translation + Vec3::Y * HEIGHT),
            if launched.wait > 0.0 {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            },
            Flying {
                course: launched.course,
                shooter: launched.shooter,
                direction: launched.direction.normalize_or(Vec2::Y),
                level: body.level(),
                damages: launched.damages,
                wait: launched.wait,
                flown: 0.0,
                spent: false,
            },
            DespawnOnExit(AppState::InGame),
        ));
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
        // A bolt that waits leaves its shooter's place as its time comes.
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
        let course = bolt.course;
        let speed = (course.speed - course.slows * bolt.flown).max(0.0);
        bolt.flown += dt;
        let step = bolt.direction * speed * dt;
        let ground = transform.translation.xz() + step;
        let cell = paths::cell_of(ground.x, ground.y);
        if bolt.flown >= course.life || !heights.walkable_on(cell, bolt.level) {
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
        let mut struck: Vec<Entity> = creatures
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
        if course.strikes == Strikes::First {
            struck.truncate(1);
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
                sure: true,
            });
        }
        if course.strikes == Strikes::FirstContact {
            bolt.spent = true;
        } else {
            commands.entity(entity).despawn();
        }
    }
}

/// The bolts.
pub struct BoltsPlugin;

impl Plugin for BoltsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Fire>()
            .add_message::<Launch>()
            .add_systems(Startup, make_looks)
            .add_systems(
                Update,
                (fire, launch, fly)
                    .chain()
                    .after(Step::Npcs)
                    .in_set(Playing),
            );
    }
}
