//! The Vampiress's sights of the hit class 862 that are no hits (`FX_VAMP_SM_HIT`; #3062;
//! docs/re/effects.md, "The hit sparks"): the blood bite's pale orange star placed once at her
//! `Bip01 Spine2` and at the bitten creature's for the bite's hold of 4.4 s (1030, `making`); the
//! master's bite's three violet ghosts of `FX_GHOST` sweeping about the bitten creature's head
//! along their clip's nine points, each of its own scale, turn, tilt and speed, for the bite's
//! seconds and a second more, each lighting the night by 150 where she stood and looping
//! `VAMP_BEHERRSCHUNG_LOOP` (1031, `making`); and the flying skull's curse at each creature it
//! curses (1033, `spells`): drops of blood born over its neck for 5 s that fall and grow, darkening
//! from red to nothing, tinted by the night and lying in the world as the neck moves on, with a
//! blood splash on the ground 20 from where it was cursed at 1.5 s and every 1 to 2 s after, the
//! violence on; without it a red glow at its neck instead and no splash; looping `VAMP_BLUTKUSS`
//! while it lives (`opensacred_rules::particles::{MasterGhost, SkullCurse}`). The stars and the
//! ghosts are added whole and untinted by the night.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::gltf::Gltf;
use bevy::prelude::*;
use opensacred_rules::particles::{
    BERSERK_POINTS, BITE_GLOW, BITE_GLOW_COLOUR, BITE_GLOW_HALF, BITE_GLOW_SECONDS, CURSE_DROPS,
    CURSE_GLOW, CURSE_GLOW_COLOUR, CURSE_GLOW_HALF, CURSE_SPLASH_COLOUR, CURSE_SPLASH_REACH,
    GHOST_LIGHT, GHOST_MODEL, GHOST_TEXTURE, MasterGhost, SkullCurse,
};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{
    Effect, Orders, Quads, empty, particle_material, particle_texture, whole_material,
};
use crate::game::{AppState, Data};
use crate::models as model_looks;
use crate::spawning::Rng;
use crate::{daynight, iso};

/// The bones the sights stand at: the bite's spine (24), the curse's neck (3) and the ghosts'
/// head (20).
const SPINE: [&str; 1] = ["Bip01 Spine2"];
const NECK: [&str; 1] = ["Bip01 Neck"];
const HEAD: &str = "Bip01 Head";
/// The master's bite's ghosts' loop, `VAMP_BEHERRSCHUNG_LOOP` (`0x793D52`), and the curse's,
/// `VAMP_BLUTKUSS` (`0x793C8E`).
const GHOST_LOOP: u32 = 2075;
const CURSE_LOOP: u32 = 2077;

/// A colour of 255ths, opaque.
fn opaque([r, g, b]: [u8; 3]) -> [f32; 4] {
    [r, g, b, 255].map(|c| f32::from(c) / 255.0)
}

/// The sights' materials and the ghosts' model, relative to the asset directory, found at the
/// first sight.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    star: Option<Handle<Effect>>,
    ghost: Option<Handle<Effect>>,
    drops: Option<Handle<Effect>>,
    glow: Option<Handle<Effect>>,
    model: Option<String>,
}

/// The sights' dice (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x862))
    }
}

/// A sight to make.
#[derive(Component)]
enum Unmade {
    /// The blood bite's star at `on`'s spine.
    Star { on: Entity },
    /// A ghost of the master's bite about `target` of the size `size` for `seconds`, its light
    /// where she stood, `at`.
    Ghost {
        target: Entity,
        size: f32,
        seconds: f32,
        at: Vec3,
    },
    /// The skull's curse at `target` of the size `size`.
    Curse { target: Entity, size: f32 },
}

/// The blood bite's star at `on`'s `Bip01 Spine2`, else its root, for the bite's hold (1030's
/// 862, `0x50FFAC` and `0x51014D`).
pub(crate) fn bite(commands: &mut Commands, on: Entity) {
    commands.spawn((Unmade::Star { on }, DespawnOnExit(AppState::InGame)));
}

/// The master's bite's three ghosts about `target`, a creature of the size `size`, for the
/// bite's `seconds`, each lighting the night where she stands, `at` (1031's 862, `0x50FC88`).
pub(crate) fn master_bite(
    commands: &mut Commands,
    target: Entity,
    (size, seconds): (f32, f32),
    at: Vec3,
) {
    for _ in 0..opensacred_rules::particles::MASTER_GHOSTS {
        commands.spawn((
            Unmade::Ghost {
                target,
                size,
                seconds,
                at,
            },
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// The skull's curse at `target`, a creature of the size `size` (1033's 862, `0x5AF08A`).
pub(crate) fn curse(commands: &mut Commands, target: Entity, size: f32) {
    commands.spawn((
        Unmade::Curse { target, size },
        DespawnOnExit(AppState::InGame),
    ));
}

/// A blood bite's star: its bearer, its place once found, its clock and its mesh.
#[derive(Component)]
struct Star {
    on: Entity,
    place: Option<Vec3>,
    clock: f32,
    mesh: Handle<Mesh>,
}

/// A master's bite's ghost: its creature, its look, its model, the model's points once found,
/// whether its clip plays, its mesh and its lamp.
#[derive(Component)]
struct Ghost {
    target: Entity,
    look: MasterGhost,
    model: Option<Entity>,
    points: Option<[Entity; 9]>,
    playing: bool,
    mesh: Handle<Mesh>,
    lamp: Entity,
}

/// A skull's curse: its creature, its look, where it was cursed and on which level, its neck's
/// place at the last frame, and its meshes of the drops and of the glow.
#[derive(Component)]
struct Curse {
    target: Entity,
    look: SkullCurse,
    origin: (Vec3, crate::terrain::Level),
    before: Option<Vec3>,
    meshes: [Handle<Mesh>; 2],
}

/// Finds the materials and the model at the first sight (`0x792890`).
fn find_looks(looks: &mut Looks, data: &Data, server: &AssetServer, effects: &mut Assets<Effect>) {
    looks.made = true;
    let assets = &data.0.assets;
    let texture = |name: &str| {
        let image = particle_texture(assets, server, name);
        if image.is_none() {
            warn!("bite sights: no texture {name}; its part is not drawn");
        }
        image
    };
    looks.star = texture(BITE_GLOW).map(|image| effects.add(whole_material(image)));
    looks.ghost = texture(GHOST_TEXTURE).map(|image| effects.add(whole_material(image)));
    looks.drops =
        texture(CURSE_DROPS).map(|image| effects.add(particle_material(image, AlphaMode::Blend)));
    looks.glow = texture(CURSE_GLOW).map(|image| effects.add(whole_material(image)));
    looks.model = crate::npcs::model_files(assets)
        .ok()
        .and_then(|files| files.get(GHOST_MODEL).cloned());
    if looks.model.is_none() {
        warn!("bite sights: no model {GHOST_MODEL}; the master's bite's ghosts are not drawn");
    }
}

/// A mesh of quads in `material`, a child of `parent`, untinted by the night where `untinted`.
fn quads_mesh(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    parent: Entity,
    (material, untinted): (Option<Handle<Effect>>, bool),
) -> Handle<Mesh> {
    let mesh = meshes.add(empty());
    if let Some(material) = material {
        let mut part = commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            NoFrustumCulling,
            ChildOf(parent),
        ));
        if untinted {
            part.insert(RenderLayers::layer(iso::UNTINTED));
        }
    }
    mesh
}

/// Gives each new sight its look, its meshes, its model and its lamp and loop (`0x7938D0`).
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    (places, ways): (Query<&GlobalTransform>, Query<&crate::crowd::InTheWay>),
    (data, server, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade) in &unmade {
        if !looks.made {
            let Some(data) = data.as_deref() else {
                return;
            };
            find_looks(&mut looks, data, &server, &mut effects);
        }
        commands.entity(entity).remove::<Unmade>();
        debug!(
            "bite sights: {}",
            match unmade {
                Unmade::Star { on } => format!("a star at {on}"),
                Unmade::Ghost { target, .. } => format!("a ghost about {target}"),
                Unmade::Curse { target, .. } => format!("a curse at {target}"),
            }
        );
        match *unmade {
            Unmade::Star { on } => {
                let mesh = quads_mesh(
                    &mut commands,
                    &mut meshes,
                    entity,
                    (looks.star.clone(), true),
                );
                commands.entity(entity).insert((
                    Star {
                        on,
                        place: None,
                        clock: 0.0,
                        mesh,
                    },
                    Transform::IDENTITY,
                    Visibility::default(),
                ));
            }
            Unmade::Ghost {
                target,
                size,
                seconds,
                at,
            } => {
                let model = looks.model.as_ref().map(|model| {
                    commands
                        .spawn((
                            model_looks::scene(&server, model),
                            Transform::IDENTITY,
                            Visibility::Hidden,
                            DespawnOnExit(AppState::InGame),
                        ))
                        .id()
                });
                let lamp = commands
                    .spawn((
                        Transform::from_translation(at.with_y(0.0)),
                        daynight::Light(GHOST_LIGHT),
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id();
                let mesh = quads_mesh(
                    &mut commands,
                    &mut meshes,
                    entity,
                    (looks.ghost.clone(), true),
                );
                let place = places.get(target).map_or(at, GlobalTransform::translation);
                commands.entity(entity).insert((
                    Ghost {
                        target,
                        look: MasterGhost::new(size, seconds, &mut dice.0),
                        model,
                        points: None,
                        playing: false,
                        mesh,
                        lamp,
                    },
                    Transform::from_translation(place),
                    Visibility::default(),
                    crate::hums::Hum(GHOST_LOOP),
                ));
            }
            Unmade::Curse { target, size } => {
                let Ok(place) = places.get(target).map(GlobalTransform::translation) else {
                    commands.entity(entity).despawn();
                    continue;
                };
                let level = ways.get(target).ok().and_then(|way| way.level);
                let meshes = [
                    quads_mesh(
                        &mut commands,
                        &mut meshes,
                        entity,
                        (looks.drops.clone(), false),
                    ),
                    quads_mesh(
                        &mut commands,
                        &mut meshes,
                        entity,
                        (looks.glow.clone(), true),
                    ),
                ];
                commands.entity(entity).insert((
                    Curse {
                        target,
                        look: SkullCurse::new(size),
                        origin: (place, level),
                        before: None,
                        meshes,
                    },
                    Transform::from_translation(place),
                    Visibility::default(),
                    crate::hums::Hum(CURSE_LOOP),
                ));
            }
        }
    }
}

/// Runs each blood bite's star a frame on and draws it (`0x793230`): placed once at its bearer's
/// spine, else its root, as it is first drawn, the same at every frame; it goes after the bite's
/// hold.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn stars(
    mut commands: Commands,
    time: Res<Time>,
    mut stars: Query<(Entity, &mut Star, &mut Transform)>,
    (places, children, bones, worn): (
        Query<&GlobalTransform>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    mut meshes: ResMut<Assets<Mesh>>,
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut star, mut transform) in &mut stars {
        star.clock += dt;
        if star.clock > BITE_GLOW_SECONDS {
            commands.entity(entity).despawn();
            continue;
        }
        if star.place.is_none() {
            star.place = crate::making::bone(star.on, &SPINE, &children, (&bones, &worn))
                .or_else(|| places.get(star.on).ok().map(GlobalTransform::translation));
        }
        let Some(place) = star.place else {
            continue;
        };
        transform.translation = place;
        let mut quads = Quads::default();
        quads.push_facing(
            Vec3::ZERO,
            BITE_GLOW_HALF * iso::MODEL_SCALE,
            [opaque(BITE_GLOW_COLOUR); 4],
        );
        if let Some(mut mesh) = meshes.get_mut(&star.mesh) {
            quads.in_order(orders.at(place), place);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// Plays each ghost's clip, looping at its speed, once its player is there, and finds its nine
/// points.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn play(
    mut commands: Commands,
    mut ghosts: Query<&mut Ghost>,
    (children, sources, names, worn): (
        Query<&Children>,
        Query<&model_looks::Source>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut players, gltfs, mut graphs): (
        Query<&mut AnimationPlayer, Without<AnimationGraphHandle>>,
        Res<Assets<Gltf>>,
        ResMut<Assets<AnimationGraph>>,
    ),
) {
    for mut ghost in &mut ghosts {
        let Some(model) = ghost.model else {
            continue;
        };
        if !ghost.playing {
            let clip = sources
                .get(model)
                .ok()
                .and_then(|source| gltfs.get(&source.0))
                .and_then(|gltf| gltf.animations.first().cloned());
            if let Some(clip) = clip {
                for entity in children.iter_descendants(model) {
                    if let Ok(mut player) = players.get_mut(entity) {
                        let (graph, node) = AnimationGraph::from_clip(clip.clone());
                        player.play(node).repeat().set_speed(ghost.look.speed());
                        commands
                            .entity(entity)
                            .insert(AnimationGraphHandle(graphs.add(graph)));
                        ghost.playing = true;
                    }
                }
            }
        }
        if ghost.points.is_none() {
            let found: Option<Vec<Entity>> = BERSERK_POINTS
                .iter()
                .map(|name| crate::making::bone_entity(model, &[name], &children, (&names, &worn)))
                .collect();
            ghost.points = found.and_then(|found| found.try_into().ok());
        }
    }
}

/// Runs each ghost a frame on (`0x792F60`) and draws it (`0x793230`): its ribbon at its creature's
/// root at its head's height, a strip of its nine pairs (`u` across, `v` along by eighths); it
/// goes, with its model and its lamp, once it has faded out, or with its creature.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn ghosts(
    mut commands: Commands,
    time: Res<Time>,
    mut ghosts: Query<(Entity, &mut Ghost, &mut Transform)>,
    (places, children, bones, worn): (
        Query<&GlobalTransform>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    mut meshes: ResMut<Assets<Mesh>>,
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut ghost, mut transform) in &mut ghosts {
        let ghost = &mut *ghost;
        ghost.look.update(dt);
        let root = places
            .get(ghost.target)
            .map(GlobalTransform::translation)
            .ok();
        let Some(root) = root.filter(|_| !ghost.look.done()) else {
            if let Some(model) = ghost.model {
                commands.entity(model).try_despawn();
            }
            commands.entity(ghost.lamp).try_despawn();
            commands.entity(entity).despawn();
            continue;
        };
        let head = crate::making::bone(ghost.target, &[HEAD], &children, (&bones, &worn))
            .map_or(root.y, |head| head.y);
        let origin = Vec3::new(root.x, head, root.z);
        transform.translation = origin;
        // Its clip's points in the model's frame, the model standing at the origin unscaled.
        let points = ghost.points.and_then(|points| {
            let mut found = [[0.0; 3]; 9];
            for (point, entity) in found.iter_mut().zip(points) {
                *point = models(places.get(entity).ok()?.translation() * s);
            }
            Some(found)
        });
        let mut quads = Quads::default();
        if let Some(ribbon) = points.map(|points| ghost.look.ribbon(&points)) {
            for k in 0..8u8 {
                let (v0, v1) = (f32::from(k) / 8.0, f32::from(k + 1) / 8.0);
                let ([a0, b0], [a1, b1]) = (ribbon[usize::from(k)], ribbon[usize::from(k + 1)]);
                quads.push_corners(
                    [world(a0), world(b0), world(b1), world(a1)],
                    [
                        Vec2::new(0.0, v0),
                        Vec2::new(1.0, v0),
                        Vec2::new(1.0, v1),
                        Vec2::new(0.0, v1),
                    ],
                    [1.0; 4],
                );
            }
        }
        if let Some(mut mesh) = meshes.get_mut(&ghost.mesh) {
            quads.in_order(orders.at(origin), origin);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// Runs each curse a frame on (`0x792F60`) and draws it (`0x793230`): its drops along its
/// creature's neck, a blood splash where one is due, with the violence on, and its drops facing
/// the camera in the ramp's colours, else its glow at the neck, after its creature's model
/// (`fx::Orders::over`); it goes with its last drop.
fn curses(
    mut commands: Commands,
    time: Res<Time>,
    mut curses: Query<(Entity, &mut Curse, &mut Transform)>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut dice, mut meshes, settings): (
        ResMut<Dice>,
        ResMut<Assets<Mesh>>,
        Option<Res<crate::settings::Settings>>,
    ),
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let violence = settings.is_some_and(|settings| settings.gameplay.violence);
    let ramp = SkullCurse::ramp();
    for (entity, mut curse, mut transform) in &mut curses {
        let curse = &mut *curse;
        // No neck, no place: nothing shows (**hypothesis**: the original's system at the frame's
        // origin, `0x5B6730`).
        let neck = crate::making::bone(curse.target, &NECK, &children, (&bones, &worn));
        let now = neck.or(curse.before).unwrap_or(curse.origin.0);
        let before = curse.before.unwrap_or(now);
        let splash = curse
            .look
            .update(dt, (models(before), models(now)), &mut dice.0);
        curse.before = Some(now);
        if curse.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation = now;
        if let Some(angle) = splash.filter(|_| violence) {
            let (sin, cos) = angle.sin_cos();
            let at =
                curse.origin.0 + world([CURSE_SPLASH_REACH * cos, CURSE_SPLASH_REACH * sin, 0.0]);
            let [r, g, b, _] = opaque(CURSE_SPLASH_COLOUR);
            crate::marks::splashes(&mut commands, (at, curse.origin.1), [r, g, b]);
        }
        let mut drops = Quads::default();
        let mut glow = Quads::default();
        if neck.is_some() || curse.before.is_some() {
            if violence {
                for drop in &curse.look.system.particles {
                    drops.push(
                        world(drop.place) - now,
                        drop.size * s,
                        drop.angle,
                        (Vec2::ZERO, 1.0),
                        ramp.at(drop.life),
                    );
                }
            } else if neck.is_some() {
                glow.push_facing(
                    Vec3::ZERO,
                    CURSE_GLOW_HALF * s,
                    [opaque(CURSE_GLOW_COLOUR); 4],
                );
            }
        }
        // Drawn after its creature's model in its cell, depth tested against it, so that the drops
        // before it veil it (seen).
        let order = orders.over(curse.target);
        for (mut quads, mesh) in [drops, glow].into_iter().zip(&curse.meshes) {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.as_drawn(order, now);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The Vampiress's sights of 862 that are no hits.
pub struct BiteSightsPlugin;

impl Plugin for BiteSightsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_systems(
                Update,
                (make, stars, play, ghosts, curses)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
