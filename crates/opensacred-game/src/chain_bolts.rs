//! The look of the chain lightning (#1092; docs/re/effects.md, "The chain lightning"): each of its
//! bolts (`spells`) from its start's `Bip01 L Hand` to its end's, of segments of
//! `cParticleSystem_freeze` (799, `0x77DB20`), two crossed quads of `FX_LIGHTNING01` each, 100
//! across, in a grey that fades after 0.4 s, added whole, their ends offset across it so that it
//! bulges between its ends (`opensacred_rules::particles::ChainBolt`), with a glow of
//! `PARTICLE_GLOW06` at its start and a light at each segment; and its impact at its end
//! (`cParticleSystem_freeze_impact`, 872), blue sparks flying off and drawn back over a glow
//! (`particles::ChainImpact`). All untinted by the night. Its first segment and its impact light
//! the models about them too, their Direct3D point lights of (0.7, 0.7, 1.0) (`fx_lights`, #2106).
//! A bolt that `spells` finds ending at a cell that may not be passed (`0x636A60`, #3158) is laid
//! up to it alone, with no impact.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use opensacred_rules::particles::{self, ChainBolt, ChainImpact};

use crate::bolt_looks::world;
use crate::frame::Playing;
use crate::fx::{Effect, Quads, empty, particle_texture, repeated_texture, whole_material};
use crate::game::{AppState, Data};
use crate::{daynight, iso};

/// The bone at each end of a bolt (`0x401000(1)`).
const HAND: [&str; 1] = ["Bip01 L Hand"];

/// How high over its place a bolt's end is where its creature has no hand, in world units
/// (**estimate**: a hand's height).
const NO_HAND: f32 = 80.0;

/// What finds a bolt's end at a creature: its place, its children, its bones and its worn items.
#[derive(SystemParam)]
pub(crate) struct Hands<'w, 's> {
    places: Query<'w, 's, &'static GlobalTransform>,
    children: Query<'w, 's, &'static Children>,
    bones: Query<'w, 's, (&'static Name, &'static GlobalTransform)>,
    worn: crate::making::Worn<'w, 's>,
}

impl Hands<'_, '_> {
    /// Where a bolt ends at `creature` (`0x44CBB0` of the bone 1, `0x44CA30`): its left hand,
    /// else over its place where it has none.
    pub(crate) fn of(&self, creature: Entity) -> Option<Vec3> {
        crate::making::bone(creature, &HAND, &self.children, (&self.bones, &self.worn)).or_else(
            || {
                self.places
                    .get(creature)
                    .ok()
                    .map(|at| at.translation() + Vec3::Y * NO_HAND)
            },
        )
    }
}

/// A bolt to make from the world point `from` to `to`: whole with its impact, or its first
/// `kept` segments alone where one ends it at a cell (`ChainBolt::laid`).
#[derive(Component)]
struct Unmade {
    from: Vec3,
    to: Vec3,
    kept: Option<usize>,
}

/// A chain lightning's bolt from the world point `from` to `to` (`spells`, [`Hands::of`]): whole
/// with its impact at `to` where `kept` is `None`, else its first `kept` segments alone.
pub(crate) fn bolt(commands: &mut Commands, (from, to): (Vec3, Vec3), kept: Option<usize>) {
    commands.spawn((Unmade { from, to, kept }, DespawnOnExit(AppState::InGame)));
}

/// A bolt: its segments and fade, its start, its frame (along it and across it, its `y` and
/// `z`), its segments' lamps, and its parts' meshes, the segments' and the glow's.
#[derive(Component)]
struct Bolt {
    bolt: ChainBolt,
    from: Vec3,
    frame: [Vec3; 3],
    lamps: Vec<Entity>,
    parts: [Handle<Mesh>; 2],
}

/// An impact: its sparks and fade, its lamp and its parts' meshes, the
/// sparks' and the glow's.
#[derive(Component)]
struct Impact {
    impact: ChainImpact,
    lamp: Entity,
    parts: [Handle<Mesh>; 2],
}

/// The materials: the segments', the glows' and the sparks'; made as the first bolt shows, none
/// where a texture is missing.
#[derive(Resource, Default)]
struct Materials(Option<[Option<Handle<Effect>>; 3]>);

/// The seed of the bolts' `rand()` (**estimate**: any).
const SEED: usize = 0x31F;

/// Spawns a part of `parent`, its mesh `mesh` of `material`, untinted.
fn part(commands: &mut Commands, parent: Entity, mesh: &Handle<Mesh>, material: Handle<Effect>) {
    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material),
        Transform::IDENTITY,
        NoFrustumCulling,
        RenderLayers::layer(iso::UNTINTED),
        ChildOf(parent),
    ));
}

/// Makes each bolt asked for (`0x526CAA`, `0x77DE80`, `0x77E630`): from hand to hand, its
/// segments laid along the line between, its impact at its end; one that a cell ends, its kept
/// segments alone.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    (data, assets, mut made, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Materials>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (entity, unmade) in &unmade {
        commands.entity(entity).despawn();
        // A bolt whose hand's own cell fails lays no segment, nor its glow and its light.
        let (Some(data), false) = (data.as_deref(), unmade.kept == Some(0)) else {
            continue;
        };
        let (from, to) = (unmade.from, unmade.to);
        let [segments, glow, sparks] = made
            .0
            .get_or_insert_with(|| {
                let shared = &data.0;
                let mut load = |image: Option<Handle<Image>>, name: &str| {
                    if image.is_none() {
                        warn!("chain bolts: no texture {name}; its part is not drawn");
                    }
                    image.map(|image| materials.add(whole_material(image)))
                };
                [
                    load(
                        repeated_texture(&shared.assets, &assets, particles::CHAIN_TEXTURE),
                        particles::CHAIN_TEXTURE,
                    ),
                    load(
                        particle_texture(&shared.assets, &assets, particles::CHAIN_GLOW_TEXTURE),
                        particles::CHAIN_GLOW_TEXTURE,
                    ),
                    load(
                        particle_texture(&shared.assets, &assets, particles::IMPACT_TEXTURE),
                        particles::IMPACT_TEXTURE,
                    ),
                ]
            })
            .clone();
        let s = iso::MODEL_SCALE;
        let along = (to - from).normalize_or(Vec3::X);
        // The template's `x` turned onto the bolt (`0x642690`), the models' `y` and `z` the
        // world's `Z` and `Y`.
        let turn = Quat::from_rotation_arc(Vec3::X, along);
        let index = usize::try_from(entity.index_u32()).unwrap_or(0);
        let mut dice = crate::seed::dice(SEED ^ index);
        let mut chain = ChainBolt::new((to - from).length() / s, &mut dice);
        if let Some(kept) = unmade.kept {
            chain.segments.truncate(kept);
        }
        let lamps = chain
            .segments
            .iter()
            .map(|segment| {
                let at = from + along * (particles::CHAIN_STEP * f32::from(segment.index) * s);
                commands
                    .spawn((
                        Transform::from_translation(at.with_y(0.0)),
                        daynight::Light(0.0),
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id()
            })
            .collect();
        let shown = commands
            .spawn((
                Transform::IDENTITY,
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let parts = [meshes.add(empty()), meshes.add(empty())];
        for (mesh, material) in parts.iter().zip([segments, glow.clone()]) {
            if let Some(material) = material {
                part(&mut commands, shown, mesh, material);
            }
        }
        // The first segment's Direct3D light, where the bolt leaves (`0x77DE80`, the event's
        // `+0x3C` 0).
        commands.spawn(crate::fx_lights::light(
            crate::fx_lights::ICE_BLUE,
            crate::fx_lights::HIGH,
            from,
            shown,
            (None, Vec3::ZERO),
        ));
        commands.entity(shown).insert(Bolt {
            bolt: chain,
            from,
            frame: [along, turn * Vec3::Z, turn * Vec3::Y],
            lamps,
            parts,
        });
        if unmade.kept.is_some() {
            continue;
        }
        let lamp = commands
            .spawn((
                Transform::from_translation(to.with_y(0.0)),
                daynight::Light(particles::IMPACT_LIGHT),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let struck = commands
            .spawn((
                Transform::from_translation(to),
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let parts = [meshes.add(empty()), meshes.add(empty())];
        for (mesh, material) in parts.iter().zip([sparks, glow]) {
            if let Some(material) = material {
                part(&mut commands, struck, mesh, material);
            }
        }
        // The impact's Direct3D light (`0x77E1C0`, `0x77E3A0`).
        commands.spawn(crate::fx_lights::child(
            crate::fx_lights::ICE_BLUE,
            crate::fx_lights::HIGH,
            struck,
        ));
        commands.entity(struck).insert(Impact {
            impact: ChainImpact::new(&mut dice),
            lamp,
            parts,
        });
    }
}

/// Runs each bolt a frame on (`0x77DAC0`, `0x77DB20`): its segments' two quads each, the
/// texture's `u` running on along it, its glow and its lamps by its fade; faded out, it goes.
fn run_bolts(
    mut commands: Commands,
    time: Res<Time>,
    mut bolts: Query<(Entity, &mut Bolt)>,
    mut lamps: Query<&mut daynight::Light>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut bolt) in &mut bolts {
        let bolt = &mut *bolt;
        bolt.bolt.update(dt);
        if bolt.bolt.done() {
            for &lamp in &bolt.lamps {
                commands.entity(lamp).despawn();
            }
            commands.entity(entity).despawn();
            continue;
        }
        let fade = bolt.bolt.fade;
        for (k, &lamp) in bolt.lamps.iter().enumerate() {
            if let Ok(mut light) = lamps.get_mut(lamp) {
                light.0 = particles::CHAIN_LIGHTS[usize::from(k > 0)] * fade;
            }
        }
        let [along, y, z] = bolt.frame;
        let grey = bolt.bolt.grey();
        let colour = [grey, grey, grey, 1.0];
        let half = particles::CHAIN_HALF * s;
        let mut segments = Quads::default();
        for segment in &bolt.bolt.segments {
            let start = particles::CHAIN_STEP * f32::from(segment.index);
            let near = bolt.from + along * (start * s);
            let far = near + along * (segment.length * s);
            let shift = |[dy, dz]: [f32; 2]| (y * dy + z * dz) * s;
            let (near, far) = (near + shift(segment.near), far + shift(segment.far));
            for (across, per) in [(y, particles::CHAIN_U[0]), (z, particles::CHAIN_U[1])] {
                let (u0, u1) = (start * per, (start + segment.length) * per);
                // Split from its near edge's top to its far edge's bottom, as the original's
                // triangles are (`+0x20F0`).
                segments.push_corners(
                    [
                        near + across * half,
                        near - across * half,
                        far - across * half,
                        far + across * half,
                    ],
                    [
                        Vec2::new(u0, 0.0),
                        Vec2::new(u0, 1.0),
                        Vec2::new(u1, 1.0),
                        Vec2::new(u1, 0.0),
                    ],
                    colour,
                );
            }
        }
        let mut glow = Quads::default();
        let g = (255.0 * fade).trunc() / 255.0;
        glow.push(
            bolt.from,
            particles::CHAIN_GLOW_HALF * s,
            0.0,
            (Vec2::ZERO, 1.0),
            [g, g, g, 1.0],
        );
        for (handle, part) in bolt.parts.iter().zip([segments, glow]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// Runs each impact a frame on (`0x77E2A0`, `0x77E3A0`): its sparks, its glow and its lamp by
/// its fade; its last spark gone, it goes.
fn run_impacts(
    mut commands: Commands,
    time: Res<Time>,
    mut impacts: Query<(Entity, &mut Impact, &GlobalTransform)>,
    mut lamps: Query<&mut daynight::Light>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let ramp = ChainImpact::ramp();
    for (entity, mut impact, place) in &mut impacts {
        let impact = &mut *impact;
        impact.impact.update(dt);
        if impact.impact.done() {
            commands.entity(impact.lamp).despawn();
            commands.entity(entity).despawn();
            continue;
        }
        if let Ok(mut light) = lamps.get_mut(impact.lamp) {
            light.0 = particles::IMPACT_LIGHT * impact.impact.fade;
        }
        let mut sparks = Quads::default();
        for spark in &impact.impact.sparks.particles {
            sparks.push(
                world(spark.place),
                spark.size * s,
                0.0,
                (Vec2::ZERO, 1.0),
                ramp.at(spark.life),
            );
        }
        let mut glow = Quads::default();
        let g = impact.impact.grey();
        glow.push(
            Vec3::ZERO,
            particles::IMPACT_GLOW_HALF * s,
            0.0,
            (Vec2::ZERO, 1.0),
            [g, g, g, 1.0],
        );
        // At its object's place in the order, as the original draws the system (#2188).
        let origin = place.translation();
        let order = orders.at(origin);
        for (handle, mut part) in impact.parts.iter().zip([sparks, glow]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, origin);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The chain lightning's looks.
pub struct ChainBoltsPlugin;

impl Plugin for ChainBoltsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Materials>().add_systems(
            Update,
            (make, run_bolts, run_impacts)
                .chain()
                .in_set(Playing)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
