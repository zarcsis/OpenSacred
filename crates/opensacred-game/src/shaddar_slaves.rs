//! Shaddar's slaves and their mark (#2919; docs/re/effects.md, "Shaddar's slaves"): the thirteen
//! FX objects 921 that the last priestess's death makes about the finale hall's pentagram
//! (`npcs`), each a star of orange rays 40 over its place that wanders as the butterflies do,
//! turning back from a cell that stops it or that neither a walker nor an arrow passes, so that
//! it flies over the lava and the pentagram (`opensacred_rules::shaddar::Slave`). A wandering
//! slave that touches the hero marks it for 5 s (the effect 32, `effects`) and flashes its rays
//! twice as long; one that touches Shaddar turns red and hunts the hero at three times its speed,
//! steering at most 3 radians a second, strikes the first creature but Shaddar it touches from
//! nobody (`fight::Burn`), and wanders again, orange; a hunter whose target is gone takes the
//! nearest living creature within some 316 units, and without one, or on a cell that neither a
//! walker nor an arrow passes, fades out. Only a hero so marked harms Shaddar (`fight`).
//!
//! The rays are camera-facing streaks of `PARTICLE_LINE01` standing on their feet, in their
//! ramp's colour by its alpha, added, untinted by the night; each slave lights the models about
//! it (`fx_lights`). A slave goes with its sector and is never made again. The marked hero wears
//! `FX_SACREDLOGO` over its head, an upright square added whole that turns about the upright by
//! the real clock, drawn after its model and depth tested against it (`fx::Orders::over`).
//!
//! A slave flies in the effects' frame (`iso::flight`), its reaches the world's on the ground.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::fight::Effect;
use opensacred_rules::shaddar::{
    self, HUNT_REACH_SQUARED, MARK, MARK_FROM, MARK_HALF, MARK_SECONDS, MARK_TEXTURE, MARK_TO,
    RAY_HALF_WIDTH, RAY_LENGTH, SHADDAR, SLAVE_LIGHT, SLAVE_SIZE, SLAVE_TEXTURE, Touched,
};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::world;
use crate::effects::Effects;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect as Material, Orders, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::terrain::Heights;
use crate::{iso, paths};

/// A slave that a script made, at its record's point on the plane of height 0.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Slave;

/// A slave flying: its rules, the creature it hunts, and its rays' mesh.
#[derive(Component)]
struct Flying {
    slave: shaddar::Slave,
    target: Option<Entity>,
    mesh: Handle<Mesh>,
}

/// The rays' and the mark's materials, made as the first slave flies; none where a texture is
/// missing.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    rays: Option<Handle<Material>>,
    mark: Option<Handle<Material>>,
}

impl Looks {
    /// Makes the materials once the game's data is there.
    fn make(
        &mut self,
        data: Option<&Data>,
        server: &AssetServer,
        materials: &mut Assets<Material>,
    ) {
        let Some(data) = data.filter(|_| !self.made) else {
            return;
        };
        self.made = true;
        let assets = &data.0.assets;
        self.rays = particle_texture(assets, server, SLAVE_TEXTURE)
            .map(|image| materials.add(particle_material(image, AlphaMode::Add)));
        self.mark = particle_texture(assets, server, MARK_TEXTURE)
            .map(|image| materials.add(crate::fx::whole_material(image)));
        if self.rays.is_none() || self.mark.is_none() {
            warn!("shaddar slaves: a texture is missing; its part is not drawn");
        }
    }
}

/// The slaves' dice (`rand()`; **estimate**: any seed).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x399))
    }
}

/// A creature that a slave may touch or hunt: its entity, its place, whom it is to the slave and
/// its size.
#[derive(Clone, Copy, Debug)]
struct Living {
    entity: Entity,
    at: Vec3,
    who: Touched,
    size: f32,
}

/// The creatures `living` that a slave at `here` touches, within its size and theirs on the
/// ground (`0x5AB490`), in their order.
fn touched(living: &[Living], here: Vec3) -> Vec<Living> {
    living
        .iter()
        .filter(|one| (one.at.xz() - here.xz()).length() < SLAVE_SIZE + one.size)
        .copied()
        .collect()
}

/// The nearest of `living` to `here` within the hunter's reach (`0x600570`), Shaddar too.
fn nearest(living: &[Living], here: Vec3) -> Option<Entity> {
    living
        .iter()
        .map(|one| (one, (one.at.xz() - here.xz()).length_squared()))
        .filter(|&(_, distance)| distance < HUNT_REACH_SQUARED)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(one, _)| one.entity)
}

/// Gives each new slave its rules, its rays' mesh and its light (`0x5A4E53`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn make(
    mut commands: Commands,
    slaves: Query<Entity, (With<Slave>, Without<Flying>)>,
    (data, server, mut looks, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Assets<Material>>,
        ResMut<Assets<Mesh>>,
    ),
    mut dice: ResMut<Dice>,
) {
    for entity in &slaves {
        looks.make(data.as_deref(), &server, &mut materials);
        let mesh = meshes.add(empty());
        let mut made = commands.entity(entity);
        if let Some(material) = looks.rays.clone() {
            made.with_child((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
        made.insert(Flying {
            slave: shaddar::Slave::new(&mut dice.0),
            target: None,
            mesh,
        });
        commands.spawn(crate::fx_lights::child(
            SLAVE_LIGHT,
            crate::fx_lights::LOW,
            entity,
        ));
        debug!("shaddar slaves: {entity} flies");
    }
}

/// Flies each slave a frame on (`0x5A6680`): wandering (`0x5A8885`) or hunting (`0x5A7DD0`), its
/// touches marking, turning or striking (`0x5ABD80`), its rays run (`0x7AA330`); ends it as its
/// last ray dies.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    (heights, rules, mut dice): (Res<Heights>, Res<crate::fight::Rules>, ResMut<Dice>),
    mut slaves: Query<(Entity, &mut Flying, &mut Transform)>,
    creatures: Query<
        (
            Entity,
            &Transform,
            &crate::crowd::InTheWay,
            &crate::fight::Health,
            Option<&crate::arrows::Size>,
            Option<&crate::ai::Mind>,
        ),
        (
            Without<crate::hero::Body>,
            Without<crate::npcs::Dies>,
            Without<Flying>,
        ),
    >,
    heroes: Query<
        (
            Entity,
            &Transform,
            &crate::hero::Body,
            &crate::fight::Health,
            Option<&crate::arrows::Size>,
        ),
        (Without<crate::npcs::Dies>, Without<Flying>),
    >,
    mut burns: MessageWriter<crate::fight::Burn>,
) {
    let dt = time.delta_secs().min(0.1);
    let own = opensacred_rules::arrows::size(0);
    // Every living creature on the ground, the heroes first.
    let heroes = heroes
        .iter()
        .filter(|(_, _, body, health, _)| body.level().is_none() && health.current > 0)
        .map(|(entity, at, _, _, size)| Living {
            entity,
            at: at.translation,
            who: Touched::Hero,
            size: size.map_or(own, |size| size.0),
        });
    let living: Vec<Living> = heroes
        .chain(
            creatures
                .iter()
                .filter(|(_, _, way, health, ..)| {
                    way.creature && way.level.is_none() && health.current > 0
                })
                .map(|(entity, at, _, _, size, mind)| Living {
                    entity,
                    at: at.translation,
                    who: if mind.is_some_and(|mind| mind.creature == SHADDAR) {
                        Touched::Shaddar
                    } else {
                        Touched::Other
                    },
                    size: size.map_or(own, |size| size.0),
                }),
        )
        .collect();
    let hero = living
        .iter()
        .find(|one| one.who == Touched::Hero)
        .map(|one| one.entity);
    let base = rules.slave_speed();
    for (entity, mut flying, mut at) in &mut slaves {
        let flying = &mut *flying;
        let here = at.translation;
        if !flying.slave.fading {
            let speed = flying.slave.speed(base);
            let step = |heading: [f32; 2]| iso::flight(heading.map(|v| v * speed * dt));
            if flying.slave.hunting {
                // Its target dropped where it lives no more; else the nearest within its reach,
                // and without one it fades.
                flying.target = flying
                    .target
                    .filter(|target| living.iter().any(|one| one.entity == *target))
                    .or_else(|| nearest(&living, here));
                match flying
                    .target
                    .and_then(|target| living.iter().find(|one| one.entity == target))
                {
                    Some(target) => flying.slave.steer(iso::in_flight(target.at - here), dt),
                    None => {
                        debug!("shaddar slaves: {entity} finds no prey and fades");
                        flying.slave.fade();
                    }
                }
                if !flying.slave.fading {
                    let next = here + step(flying.slave.heading);
                    at.translation = next;
                    let homing = flying.target.is_some();
                    act(
                        entity,
                        flying,
                        &touched(&living, next),
                        homing,
                        (&rules, hero),
                        (&mut commands, &mut burns),
                    );
                    if !heights.passes(paths::cell_of(next.x, next.z), None) {
                        debug!("shaddar slaves: {entity} hunts onto a cell it may not cross");
                        flying.slave.fade();
                    }
                }
            } else {
                let next = here + step(flying.slave.heading);
                flying.slave.wander(dt);
                act(
                    entity,
                    flying,
                    &touched(&living, next),
                    false,
                    (&rules, hero),
                    (&mut commands, &mut burns),
                );
                let cell = paths::cell_of(next.x, next.z);
                if heights.passes(cell, None) && !heights.stops_flight(cell) {
                    at.translation = next;
                } else {
                    flying.slave.bounce(&mut dice.0);
                }
            }
        }
        if !flying.slave.update(dt, &mut dice.0) {
            debug!("shaddar slaves: {entity} ends");
            commands.entity(entity).despawn();
        }
    }
}

/// What a slave does at its touches `touched` (`0x5ABD80`, the case `0x399`): marks the heroes
/// for 5 s, strikes from nobody, or turns to hunt the hero `hero`; a strike drops its target.
fn act(
    entity: Entity,
    flying: &mut Flying,
    touched: &[Living],
    homing: bool,
    (rules, hero): (&crate::fight::Rules, Option<Entity>),
    (commands, burns): (&mut Commands, &mut MessageWriter<crate::fight::Burn>),
) {
    if touched.is_empty() {
        return;
    }
    let who: Vec<Touched> = touched.iter().map(|one| one.who).collect();
    let deeds = flying.slave.touch(&who, homing);
    for &index in &deeds.marks {
        let marked = touched[index].entity;
        commands
            .entity(marked)
            .entry::<Effects>()
            .or_default()
            .and_modify(|mut effects| effects.add_alone(Effect::new(MARK, 0.0), MARK_SECONDS));
    }
    for &index in &deeds.strikes {
        let target = touched[index].entity;
        debug!("shaddar slaves: {entity} strikes {target}");
        burns.write(crate::fight::Burn {
            target,
            damages: rules.slave_damages(),
        });
    }
    if !deeds.strikes.is_empty() {
        flying.target = None;
    }
    if deeds.hunts_hero {
        debug!("shaddar slaves: {entity} touches Shaddar and hunts the hero");
        flying.target = hero;
    }
}

/// Draws each slave's rays (`0x7AA4B0`): a streak of each particle standing on its foot at its
/// angle in the screen's plane, `0.25·s` wide and `2·s` long, in its ramp's colour, at its
/// place in the order.
fn draw(slaves: Query<(&Flying, &Transform)>, orders: Orders, mut meshes: ResMut<Assets<Mesh>>) {
    let s = iso::MODEL_SCALE;
    for (flying, at) in &slaves {
        let slave = &flying.slave;
        let ramp = shaddar::Slave::ramp(slave.red);
        let mut quads = Quads::default();
        for ray in &slave.system.particles {
            quads.push_ray(
                world(ray.place),
                (
                    RAY_HALF_WIDTH * ray.size * s,
                    RAY_LENGTH * ray.size * s,
                    0.0,
                ),
                ray.angle,
                ramp.at(ray.life),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&flying.mesh) {
            quads.in_order(orders.at(at.translation), at.translation);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// A marked creature's emblem: its bearer and its quad's mesh.
#[derive(Component)]
struct Mark {
    bearer: Entity,
    mesh: Handle<Mesh>,
}

/// The entity of a bearer's emblem.
#[derive(Component)]
struct Wears(Entity);

/// Gives each creature that comes to hold the effect 32 its emblem, and takes it from one that
/// no longer does (`cCreature_render` at `0x599FCE`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn mark(
    mut commands: Commands,
    bearers: Query<(Entity, &Effects, Option<&Wears>), Changed<Effects>>,
    (data, server, mut looks, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Assets<Material>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (bearer, effects, wears) in &bearers {
        match (effects.holds(MARK), wears) {
            (true, None) => {
                looks.make(data.as_deref(), &server, &mut materials);
                let mesh = meshes.add(empty());
                let mut mark = commands.spawn((
                    Transform::IDENTITY,
                    Visibility::default(),
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                    DespawnOnExit(AppState::InGame),
                ));
                if let Some(material) = looks.mark.clone() {
                    mark.insert((Mesh3d(mesh.clone()), MeshMaterial3d(material)));
                }
                let mark = mark.insert(Mark { bearer, mesh }).id();
                commands.entity(bearer).insert(Wears(mark));
                debug!("shaddar slaves: {bearer} wears the mark");
            }
            (false, Some(&Wears(mark))) => {
                commands.entity(mark).despawn();
                commands.entity(bearer).remove::<Wears>();
                debug!("shaddar slaves: {bearer}'s mark goes");
            }
            _ => {}
        }
    }
}

/// The emblem's corners about its bearer's head at the turn `turn`: from `MARK_HALF` to each
/// side and from `MARK_FROM` to `MARK_TO` up, turned about the upright from the screen's
/// horizontal toward the viewer; from the bottom left, anticlockwise.
fn emblem(head: Vec3, turn: f32) -> [Vec3; 4] {
    let s = iso::MODEL_SCALE;
    let (across, down) = iso::fx_axes();
    let (sin, cos) = turn.sin_cos();
    let side = (across * cos + down * sin) * MARK_HALF * s;
    let [from, to] = [MARK_FROM, MARK_TO].map(|height| head + Vec3::Y * height * s);
    [from - side, from + side, to + side, to - side]
}

/// Lays each emblem over its bearer's head at the real clock's turn (`0x40A690`), where the
/// head's texels lie in the order; none while the bearer is hidden, and the emblem goes with its
/// bearer.
fn keep(
    mut commands: Commands,
    mut marks: Query<(Entity, &Mark, &mut Transform)>,
    bearers: Query<(&GlobalTransform, Option<&InheritedVisibility>)>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut meshes, orders, time): (ResMut<Assets<Mesh>>, Orders, Res<Time<Real>>),
) {
    let millis = u64::try_from(time.elapsed().as_millis()).unwrap_or(0);
    let turn = shaddar::mark_turn(millis);
    for (mark, look, mut at) in &mut marks {
        let Ok((root, shown)) = bearers.get(look.bearer) else {
            commands.entity(mark).despawn();
            continue;
        };
        let root = root.translation();
        at.translation = root;
        let mut quads = Quads::default();
        let head = crate::making::bone(look.bearer, &["Bip01 Head"], &children, (&bones, &worn));
        if let Some(head) = head.filter(|_| shown.is_none_or(|shown| shown.get())) {
            quads.push_corners(
                emblem(head - root, turn),
                [Vec2::Y, Vec2::ONE, Vec2::X, Vec2::ZERO],
                [1.0; 4],
            );
            quads.as_drawn(orders.over(look.bearer), root);
        }
        if let Some(mut mesh) = meshes.get_mut(&look.mesh) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// Shaddar's slaves and their mark.
pub struct ShaddarSlavesPlugin;

impl Plugin for ShaddarSlavesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (make, fly, draw)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                Update,
                (mark, keep)
                    .chain()
                    .in_set(Playing)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(id: u32, x: f32, who: Touched, size: f32) -> Living {
        Living {
            entity: Entity::from_raw_u32(id).unwrap(),
            at: Vec3::new(x, 7.0, 0.0),
            who,
            size,
        }
    }

    #[test]
    fn a_slave_touches_within_its_size_and_theirs_and_hunts_the_nearest() {
        let living = [
            one(1, 140.0, Touched::Hero, 50.0),
            one(2, -99.0, Touched::Other, 50.0),
            one(3, -310.0, Touched::Shaddar, 50.0),
        ];
        let near = touched(&living, Vec3::ZERO);
        assert_eq!(near.len(), 1);
        assert_eq!(near[0].who, Touched::Other);
        // A bigger creature is touched further; one at the sum of the sizes is not.
        assert_eq!(
            touched(&[one(4, 140.0, Touched::Hero, 91.0)], Vec3::ZERO).len(),
            1
        );
        assert!(touched(&[one(5, 100.0, Touched::Hero, 50.0)], Vec3::ZERO).is_empty());
        assert_eq!(nearest(&living, Vec3::ZERO), Some(living[1].entity));
        assert_eq!(
            nearest(&living, Vec3::new(-320.0, 0.0, 0.0)),
            Some(living[2].entity)
        );
        // Some 316 units at most.
        assert_eq!(nearest(&living[..1], Vec3::new(-180.0, 0.0, 0.0)), None);
        assert_eq!(
            nearest(&living[..1], Vec3::new(-170.0, 0.0, 0.0)),
            Some(living[0].entity)
        );
    }

    #[test]
    fn the_emblem_stands_over_the_head_and_turns_about_the_upright() {
        let s = iso::MODEL_SCALE;
        let head = Vec3::new(3.0, 50.0, -4.0);
        let [bottom_left, bottom_right, top_right, top_left] = emblem(head, 0.0);
        assert!((bottom_left.y - head.y - 12.0 * s).abs() < 1e-3);
        assert!((top_left.y - head.y - 42.0 * s).abs() < 1e-3);
        assert!(((bottom_right - bottom_left).length() - 30.0 * s).abs() < 1e-3);
        assert!((top_right - bottom_right).xz().length() < 1e-4);
        assert!(
            ((bottom_left + bottom_right) / 2.0)
                .xz()
                .distance(head.xz())
                < 1e-4
        );
        // Across the screen at 0, edge-on a quarter turn later.
        let across = |a: Vec3, b: Vec3| (b - a).dot(iso::right());
        assert!((across(bottom_left, bottom_right) - 30.0 * s).abs() < 1e-3);
        let [left, right, ..] = emblem(head, std::f32::consts::FRAC_PI_2);
        assert!(across(left, right).abs() < 1e-3);
    }
}
