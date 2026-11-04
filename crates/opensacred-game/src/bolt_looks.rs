//! The bolts' looks (#1092's first family; docs/re/effects.md, "The bolts"), in place of the
//! stand-ins' balls, drawn as the original's particle systems and untinted by the night
//! (`iso::UNTINTED`):
//!
//! - the railgun's (801, `cParticleSystem_railgun`): two crossed quads of `FX_PLASMA_CENTER`
//!   along its flight, laid over by their alpha, and for the hero's bolt a line of eight dim blue
//!   glows behind it; gone at its hit or its end;
//! - the BFG's (875, `cParticleSystem_bfg`): three starbursts `FX_BFG_1` to `3` that pulse against
//!   one another, and a cyan glow on the ground under it;
//! - the Mage's fireball (778, `cParticleSystem_firestrike`): a trail of `PARTICLE_FIRE01` flames
//!   left in the world along its way, each playing its flip-book once, a flame more at its hit,
//!   the trail burning out after it;
//! - the Daemon's 1103 (1159, `cParticleSystem_emperor`): rays of `PARTICLE_LIGHTNING01` rooted
//!   at its centre and a flickering glow, flaring 2 s on its target's neck at its hit.
//!
//! Each look is an entity of its own that follows its bolt and outlives it as its system does;
//! sizes in the models' units are drawn at the world's scale (`iso::MODEL_SCALE`). The railgun's,
//! the BFG's and the fireball's light the night by their systems' radii while they live, at their
//! places on the ground at height 0 (`0x5AA26E`, `0x41B200`; #61). Not followed: the fireball's
//! Direct3D point light on the models, and the looks of a caster gone (`+0xA9` 1: a black
//! railgun, a green fireball).

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use opensacred_rules::particles::{self, Emperor, Firestrike};
use opensacred_rules::spawning::Rng;

use crate::frame::{Playing, Step};
use crate::fx::{Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::{daynight, iso};

/// What a bolt looks like.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Railgun,
    Bfg,
    Fireball,
    Emperor,
}

/// How a bolt ended (`bolts`): where, and the creature it hit first if it hit one, or whether it
/// flew out its time rather than stopping at a cell it may not pass.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Ended {
    pub bolt: Entity,
    pub at: Vec3,
    pub hit: Option<Entity>,
    pub timed: bool,
}

/// A look to make for a bolt ([`make`]).
#[derive(Component)]
struct Unmade {
    bolt: Entity,
    kind: Kind,
    direction: Vec2,
    hero: bool,
}

/// Makes the look of the bolt `bolt` of the kind `kind`, flying along `direction` on the ground;
/// `hero` where its shooter is the hero (the railgun's glows are a hero's alone, `+0xA9` 0).
pub fn spawn(commands: &mut Commands, bolt: Entity, kind: Kind, direction: Vec2, hero: bool) {
    commands.spawn((
        Unmade {
            bolt,
            kind,
            direction: direction.normalize_or(Vec2::Y),
            hero,
        },
        Transform::IDENTITY,
        Visibility::Hidden,
        DespawnOnExit(AppState::InGame),
    ));
}

/// A look: its bolt while it flies, its direction, whether a hero shot it, its clock, its place
/// in the world and the place before, what its system holds, the creature it follows after a
/// hit, the meshes of its parts, and its light's entity and radius, where it has one.
#[derive(Component)]
struct Look {
    bolt: Option<Entity>,
    direction: Vec2,
    hero: bool,
    clock: f32,
    place: Vec3,
    before: Option<Vec3>,
    system: System,
    follows: Option<Entity>,
    parts: Vec<Handle<Mesh>>,
    light: Option<(Entity, f32)>,
}

/// A look's light, at its place on the ground at height 0.
#[derive(Component)]
struct Lamp;

/// The radius of a kind's light (`+0x9C`), none for the Daemon's bolt.
fn light_of(kind: Kind) -> Option<f32> {
    match kind {
        Kind::Railgun => Some(particles::RAILGUN_LIGHT),
        Kind::Bfg => Some(particles::BFG_LIGHT),
        Kind::Fireball => Some(particles::FIRESTRIKE_LIGHT),
        Kind::Emperor => None,
    }
}

/// A look's system.
enum System {
    Railgun,
    Bfg,
    Fireball(Firestrike),
    Emperor(Emperor),
}

/// The textures of a kind's parts and how each is drawn: laid over by its alpha, or added.
fn parts(kind: Kind) -> &'static [(&'static str, AlphaMode)] {
    match kind {
        Kind::Railgun => &[
            ("FX_PLASMA_CENTER", AlphaMode::Blend),
            ("PARTICLE_GLOW01", AlphaMode::Add),
        ],
        Kind::Bfg => &[
            ("FX_BFG_1", AlphaMode::Add),
            ("FX_BFG_2", AlphaMode::Add),
            ("FX_BFG_3", AlphaMode::Add),
            ("PARTICLE_GLOW03", AlphaMode::Add),
        ],
        Kind::Fireball => &[(particles::FIRESTRIKE_TEXTURE, AlphaMode::Add)],
        Kind::Emperor => &[
            (particles::EMPEROR_TEXTURE, AlphaMode::Add),
            (particles::EMPEROR_GLOW_TEXTURE, AlphaMode::Add),
        ],
    }
}

/// The parts' materials by their texture, made as the first is asked; none where the texture is
/// missing.
#[derive(Resource, Default)]
struct Materials(HashMap<&'static str, Option<Handle<crate::fx::Effect>>>);

/// The looks' rolls: their particles' and the glow's flicker.
#[derive(Resource)]
struct Rolls(Rng);

impl Default for Rolls {
    fn default() -> Self {
        Self(crate::seed::dice(0x1092))
    }
}

/// Gives each new look its system, its parts' meshes and their materials.
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    data: Option<Res<Data>>,
    assets: Res<AssetServer>,
    (mut made, mut materials, mut meshes): (
        ResMut<Materials>,
        ResMut<Assets<crate::fx::Effect>>,
        ResMut<Assets<Mesh>>,
    ),
    places: Query<&Transform>,
) {
    for (entity, unmade) in &unmade {
        let system = match unmade.kind {
            Kind::Railgun => System::Railgun,
            Kind::Bfg => System::Bfg,
            Kind::Fireball => System::Fireball(Firestrike::new()),
            Kind::Emperor => System::Emperor(Emperor::new()),
        };
        let place = places
            .get(unmade.bolt)
            .map_or(Vec3::ZERO, |at| at.translation);
        let mut look = commands.entity(entity);
        let mut handles = Vec::new();
        for &(texture, mode) in parts(unmade.kind) {
            let material = made
                .0
                .entry(texture)
                .or_insert_with(|| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, texture));
                    if image.is_none() {
                        warn!("bolt_looks: no texture {texture}; its part is not drawn");
                    }
                    image.map(|image| materials.add(particle_material(image, mode)))
                })
                .clone();
            let mesh = meshes.add(empty());
            handles.push(mesh.clone());
            if let Some(material) = material {
                look.with_child((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    bevy::camera::visibility::NoFrustumCulling,
                    bevy::camera::visibility::RenderLayers::layer(iso::UNTINTED),
                ));
            }
        }
        let light = light_of(unmade.kind).map(|radius| {
            let lamp = look
                .commands()
                .spawn((
                    Lamp,
                    Transform::from_translation(place.with_y(0.0)),
                    ChildOf(entity),
                ))
                .id();
            (lamp, radius)
        });
        look.remove::<Unmade>().insert(Look {
            bolt: Some(unmade.bolt),
            direction: unmade.direction,
            hero: unmade.hero,
            clock: 0.0,
            place,
            before: None,
            system,
            follows: None,
            parts: handles,
            light,
        });
    }
}

/// Answers each bolt's end: the railgun's and the BFG's looks go with it; the fireball bursts at
/// a hit or a stop and burns out, or burns out at its time's end; the Daemon's flares on the
/// creature it hit, or where it stopped.
fn end(
    mut commands: Commands,
    mut ended: MessageReader<Ended>,
    mut looks: Query<(Entity, &mut Look)>,
) {
    for ended in ended.read() {
        for (entity, mut look) in &mut looks {
            if look.bolt != Some(ended.bolt) {
                continue;
            }
            look.bolt = None;
            look.place = ended.at;
            match &mut look.system {
                System::Railgun | System::Bfg => commands.entity(entity).despawn(),
                System::Fireball(fireball) if ended.timed => fireball.out(),
                System::Fireball(fireball) => fireball.hit(),
                System::Emperor(emperor) => {
                    emperor.flare();
                    look.follows = ended.hit;
                }
            }
        }
    }
}

/// The bones that the Daemon's bolt flares on after a hit, the neck's (bone 3).
const NECK: [&str; 1] = ["Bip01 Neck"];

/// Runs each look a frame on and draws it where its bolt flies, or where it ended, its light
/// under it while it shows.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries, with their filters, as arguments"
)]
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut looks: Query<(Entity, &mut Look, &mut Visibility)>,
    bolts: Query<(&Transform, &Visibility), (Without<Look>, Without<Lamp>)>,
    mut lamps: Query<&mut Transform, With<Lamp>>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    mut meshes: ResMut<Assets<Mesh>>,
    mut rolls: ResMut<Rolls>,
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut look, mut visibility) in &mut looks {
        let look = &mut *look;
        // While its bolt flies, it follows it, hidden as it waits.
        if let Some(bolt) = look.bolt {
            let Ok((at, shown)) = bolts.get(bolt) else {
                // Gone without its end told: the look with it.
                commands.entity(entity).despawn();
                continue;
            };
            look.place = at.translation;
            if *shown == Visibility::Hidden {
                *visibility = Visibility::Hidden;
                look.before = None;
                if let Some((lamp, _)) = look.light {
                    commands.entity(lamp).remove::<daynight::Light>();
                }
                continue;
            }
        } else if let Some(target) = look.follows
            && let Some(neck) = crate::making::bone(target, &NECK, &children, (&bones, &worn))
        {
            look.place = neck;
        }
        *visibility = Visibility::Inherited;
        if let Some((lamp, radius)) = look.light
            && let Ok(mut at) = lamps.get_mut(lamp)
        {
            at.translation = look.place.with_y(0.0);
            commands.entity(lamp).insert(daynight::Light(radius));
        }
        look.clock += dt;
        let before = look.before.unwrap_or(look.place);
        look.before = Some(look.place);
        let mut quads: Vec<Quads> = look.parts.iter().map(|_| Quads::default()).collect();
        let done = match &mut look.system {
            System::Railgun => {
                railgun(look.place, look.direction, look.hero, &mut quads);
                false
            }
            System::Bfg => {
                bfg(look.place, look.clock, &mut quads);
                false
            }
            System::Fireball(fireball) => {
                fireball.update(dt, (models(before), models(look.place)), &mut rolls.0);
                if let Some(part) = quads.first_mut() {
                    flames(
                        &fireball.system,
                        particles::FIRESTRIKE_GRID,
                        Vec3::ZERO,
                        part,
                    );
                }
                fireball.done()
            }
            System::Emperor(emperor) => {
                emperor.update(dt, &mut rolls.0);
                #[expect(clippy::cast_precision_loss, reason = "a roll below 1000")]
                let roll = rolls.0.below(1000) as f32 / 1000.0;
                rays(emperor, look.place, roll, &mut quads);
                emperor.done()
            }
        };
        if done {
            commands.entity(entity).despawn();
            continue;
        }
        for (handle, part) in look.parts.iter().zip(quads) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// A world point in the models' frame, its third coordinate up.
pub(crate) fn models(point: Vec3) -> [f32; 3] {
    let s = iso::MODEL_SCALE;
    [point.x / s, point.z / s, point.y / s]
}

/// A point of the models' frame in the world.
pub(crate) fn world([x, y, z]: [f32; 3]) -> Vec3 {
    Vec3::new(x, z, y) * iso::MODEL_SCALE
}

/// The railgun's parts (`0x77EBC0`): its crossed quads along `direction` at `place`, the texture's
/// `u` from its head forward to its tail, and for a hero's bolt its line of glows behind it.
fn railgun(place: Vec3, direction: Vec2, hero: bool, quads: &mut [Quads]) {
    let s = iso::MODEL_SCALE;
    let along = Vec3::new(direction.x, 0.0, direction.y);
    let across = Vec3::Y.cross(along).normalize_or(Vec3::X);
    let (length, width) = particles::RAILGUN_HALF;
    let white = [1.0; 4];
    if let Some(body) = quads.first_mut() {
        for side in [across, Vec3::Y] {
            let corner = |x: f32, w: f32| place + (along * x + side * w) * s;
            body.push_corners(
                [
                    corner(length, width),
                    corner(-length, width),
                    corner(-length, -width),
                    corner(length, -width),
                ],
                [
                    Vec2::new(0.0, 0.0),
                    Vec2::new(1.0, 0.0),
                    Vec2::new(1.0, 1.0),
                    Vec2::new(0.0, 1.0),
                ],
                white,
            );
        }
    }
    if hero && let Some(glows) = quads.get_mut(1) {
        for i in 0..particles::RAILGUN_GLOWS {
            #[expect(clippy::cast_precision_loss, reason = "a glow's index, below 8")]
            let step = particles::RAILGUN_GLOW_STEP * i as f32;
            glows.push(
                place + along * (step - particles::RAILGUN_GLOW_BEHIND) * s,
                particles::RAILGUN_GLOW_HALF * s,
                0.0,
                (Vec2::ZERO, 1.0),
                particles::RAILGUN_GLOW_COLOUR,
            );
        }
    }
}

/// The BFG's parts (`0x77F290`): its three glows in their greys of its clock `clock` at `place`,
/// and its glow on the ground under it, laid flat.
fn bfg(place: Vec3, clock: f32, quads: &mut [Quads]) {
    let s = iso::MODEL_SCALE;
    for (part, grey) in quads.iter_mut().zip(particles::bfg_greys(clock)) {
        part.push(
            place,
            particles::BFG_GLOW_HALF * s,
            0.0,
            (Vec2::ZERO, 1.0),
            [grey, grey, grey, 1.0],
        );
    }
    if let Some(ground) = quads.get_mut(3) {
        let at = place - Vec3::Y * (crate::bolts::HEIGHT - GROUND_LIFT);
        let half = particles::BFG_GROUND_HALF * s;
        ground.push_corners(
            [
                at + Vec3::new(-half, 0.0, -half),
                at + Vec3::new(half, 0.0, -half),
                at + Vec3::new(half, 0.0, half),
                at + Vec3::new(-half, 0.0, half),
            ],
            [
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(1.0, 1.0),
                Vec2::new(0.0, 1.0),
            ],
            particles::BFG_GROUND_COLOUR,
        );
    }
}

/// How far over the ground the BFG's ground glow lies, in world units, so that the ground's own
/// faces do not cut it.
const GROUND_LIFT: f32 = 0.5;

/// The flames of `system` about the world point `origin` of its frame (`0x76BDD0`, `0x775050`):
/// white, turned, each its frame of the `grid` × `grid` by its age; the fireball's, in the world,
/// and the torches' (`torches`).
pub(crate) fn flames(system: &particles::System, grid: u8, origin: Vec3, part: &mut Quads) {
    let side = 1.0 / f32::from(grid);
    for flame in &system.particles {
        let frame = particles::age_frame(flame.life, grid);
        let corner = Vec2::new(f32::from(frame % grid), f32::from(frame / grid)) * side;
        part.push(
            origin + world(flame.place),
            flame.size * iso::MODEL_SCALE,
            flame.angle,
            (corner, side),
            [1.0; 4],
        );
    }
}

/// The Daemon's bolt's rays about `place`, rooted at it and turned about their feet, in its ramp,
/// and while it makes them its glow of the half side its roll `roll` gives (`0x7BDB50`).
fn rays(emperor: &Emperor, place: Vec3, roll: f32, quads: &mut [Quads]) {
    let ramp = Emperor::ramp();
    let grid = particles::EMPEROR_GRID;
    let side = 1.0 / f32::from(grid);
    if let Some(part) = quads.first_mut() {
        for ray in &emperor.system.particles {
            let frame = ray.frame % (grid * grid);
            let corner = Vec2::new(f32::from(frame % grid), f32::from(frame / grid)) * side;
            part.push_footed(
                place + world(ray.place),
                ray.size * iso::MODEL_SCALE,
                ray.angle,
                (corner, side),
                ramp.at(ray.life),
            );
        }
    }
    if emperor.emits()
        && let Some(glow) = quads.get_mut(1)
    {
        glow.push(
            place,
            emperor.glow_half(roll) * iso::MODEL_SCALE,
            0.0,
            (Vec2::ZERO, 1.0),
            particles::EMPEROR_GLOW_COLOUR,
        );
    }
}

/// The bolts' looks.
pub struct BoltLooksPlugin;

impl Plugin for BoltLooksPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Ended>()
            .init_resource::<Materials>()
            .init_resource::<Rolls>()
            .add_systems(
                Update,
                (make, end, run)
                    .chain()
                    .after(crate::bolts::Flown)
                    .after(Step::Npcs)
                    .in_set(Playing)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bolts_light_by_their_systems_but_the_daemons() {
        assert_eq!(light_of(Kind::Railgun), Some(100.0));
        assert_eq!(light_of(Kind::Bfg), Some(120.0));
        assert_eq!(light_of(Kind::Fireball), Some(200.0));
        assert_eq!(light_of(Kind::Emperor), None);
    }

    #[test]
    fn a_point_goes_to_the_models_frame_and_back() {
        let point = Vec3::new(120.0, 80.0, -45.0);
        let back = world(models(point));
        assert!((back - point).length() < 1e-3, "{back}");
        // The models' third coordinate is the world's up.
        assert!((models(Vec3::Y * iso::MODEL_SCALE)[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_railguns_quads_lie_along_its_flight() {
        let mut quads = vec![Quads::default(), Quads::default()];
        railgun(Vec3::ZERO, Vec2::X, false, &mut quads);
        // No glows for a creature's bolt.
        let mut mesh = empty();
        std::mem::take(&mut quads[1]).into_mesh(&mut mesh);
        assert_eq!(mesh.indices().map(|indices| indices.len()), Some(6));
        let mut mesh = empty();
        std::mem::take(&mut quads[0]).into_mesh(&mut mesh);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("no positions");
        };
        // Two quads, 40 long along x, 30 wide, one across and one upright.
        assert_eq!(positions.len(), 8);
        let s = iso::MODEL_SCALE;
        assert!((positions[0][0] - 20.0 * s).abs() < 1e-3);
        assert!(positions[..4].iter().all(|p| p[1].abs() < 1e-3));
        assert!(positions[4..].iter().all(|p| p[2].abs() < 1e-3));
    }
}
