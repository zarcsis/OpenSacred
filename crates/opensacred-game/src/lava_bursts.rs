//! The lava bursts (877, `FX_LAVABURST`, #1964; docs/re/effects.md, "The lava bursts"): while a
//! cell of deep lava is in view (`liquids`, [`Vents`]), at most once a second one of them throws a
//! rock of lava in an arc (`opensacred_rules::particles::LavaBurst`, `GIBS01.GRN` in `C_LAVA00`,
//! lit as the world's models are) with `LAVABURST` there, a glow at it and a trail of glows, and a
//! small fire where it leaves and where it lands (`opensacred_rules::particles::LavaFlames`); the
//! glows and the flames are added and untinted by the night, and the flames light it.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, LavaBurst, LavaFlames};

use crate::frame::{Playing, Unpaused};
use crate::fx::{
    Effect, Quads, empty, particle_material, particle_texture, texture_path, whole_material,
};
use crate::game::{AppState, Data};
use crate::npcs::Retexture;
use crate::sfx::{Of, PlayAt, Roll};
use crate::spawning::Rng;
use crate::terrain::Heights;
use crate::{bolt_looks, daynight, iso, models, npcs};

/// The middles of a sector's cells of deep lava (`liquids`), whence the bursts come; it goes with
/// its sector.
#[derive(Component)]
pub struct Vents(pub Vec<Vec3>);

/// The seconds before the next burst may come (the renderer's `+0x9E08`).
#[derive(Resource, Default)]
struct Cooldown(f32);

/// The bursts' dice: their rolls, their arcs and their flames.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x877))
    }
}

/// The rock's model and texture, relative to the asset directory, and the materials of the trail,
/// the glow and the flames; made at the first burst, none where a texture is missing.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    model: Option<String>,
    texture: Option<String>,
    trail: Option<Handle<Effect>>,
    glow: Option<Handle<Effect>>,
    flames: Option<Handle<Effect>>,
}

/// A burst in flight: its system in the frame of its vent, its rock, and the meshes of its trail
/// and its glow.
#[derive(Component)]
struct Burst {
    burst: LavaBurst,
    rock: Option<Entity>,
    trail: Handle<Mesh>,
    glow: Handle<Mesh>,
}

/// A burst's small fire: its flames in the frame of its place, and their mesh.
#[derive(Component)]
struct Flames {
    flames: LavaFlames,
    mesh: Handle<Mesh>,
}

/// A mesh drawn under `parent` of `material`, untinted by the night.
fn part(
    commands: &mut Commands,
    parent: Entity,
    meshes: &mut Assets<Mesh>,
    material: Option<&Handle<Effect>>,
) -> Handle<Mesh> {
    let mesh = meshes.add(empty());
    if let Some(material) = material {
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::IDENTITY,
            NoFrustumCulling,
            RenderLayers::layer(iso::UNTINTED),
            ChildOf(parent),
        ));
    }
    mesh
}

/// Lays a burst's small fire on the ground at `point`.
fn flames(
    commands: &mut Commands,
    point: Vec3,
    looks: &Looks,
    (meshes, dice): (&mut Assets<Mesh>, &mut Rng),
) {
    let entity = commands
        .spawn((
            Transform::from_translation(point),
            Visibility::default(),
            daynight::Light(particles::LAVA_FLAMES_LIGHT),
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    let mesh = part(commands, entity, meshes, looks.flames.as_ref());
    commands.entity(entity).insert(Flames {
        flames: LavaFlames::new(dice),
        mesh,
    });
}

/// Rolls a burst at each vent in view while none has come for a second (`0x62B7FA`, `0x41AF60`):
/// each vent's roll in turn, the first that wins throws a rock from it, with its sound and a small
/// fire there.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries in groups"
)]
fn vent(
    mut commands: Commands,
    time: Res<Time>,
    vents: Query<&Vents>,
    cameras: Query<(&Camera, &GlobalTransform), (With<daynight::Night>, Without<iso::Untinted>)>,
    (data, assets): (Option<Res<Data>>, Res<AssetServer>),
    (mut cooldown, mut dice, mut looks, mut materials, mut meshes, mut sounds): (
        ResMut<Cooldown>,
        ResMut<Dice>,
        ResMut<Looks>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
        MessageWriter<PlayAt>,
    ),
) {
    if cooldown.0 > 0.0 {
        cooldown.0 -= time.delta_secs();
        return;
    }
    let Some((camera, view)) = cameras.iter().next() else {
        return;
    };
    let Some(size) = camera.logical_viewport_size() else {
        return;
    };
    let in_view = |point: Vec3| {
        camera
            .world_to_viewport(view, point)
            .is_ok_and(|p| (0.0..=size.x).contains(&p.x) && (0.0..=size.y).contains(&p.y))
    };
    let Some(point) = vents
        .iter()
        .flat_map(|vents| vents.0.iter().copied())
        .filter(|&point| in_view(point))
        .find(|_| particles::lava_bursts(&mut dice.0))
    else {
        return;
    };
    cooldown.0 = particles::LAVA_BURST_COOLDOWN;
    let Some(data) = data.as_deref() else {
        return;
    };
    if !looks.made {
        let shared = &data.0;
        looks.made = true;
        looks.model = npcs::model_files(&shared.assets)
            .ok()
            .and_then(|files| files.get(particles::METEOR_MODEL).cloned());
        looks.texture = texture_path(&shared.assets, particles::METEOR_TEXTURE);
        let load = |name: &str| {
            let image = particle_texture(&shared.assets, &assets, name);
            if image.is_none() {
                warn!("lava bursts: no texture {name}; its part is not drawn");
            }
            image
        };
        let glow = load(particles::METEOR_GLOW_TEXTURE);
        let fire = load(particles::LAVA_FLAMES_TEXTURE);
        looks.trail = glow
            .clone()
            .map(|image| materials.add(particle_material(image, AlphaMode::Add)));
        looks.glow = glow.map(|image| materials.add(whole_material(image)));
        looks.flames = fire.map(|image| materials.add(particle_material(image, AlphaMode::Add)));
    }
    debug!("lava bursts: a burst at {point}");
    let shown = commands
        .spawn((
            Transform::from_translation(point),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    let trail = part(&mut commands, shown, &mut meshes, looks.trail.as_ref());
    let glow = part(&mut commands, shown, &mut meshes, looks.glow.as_ref());
    let rock = looks.model.as_ref().map(|model| {
        let mut rock = commands.spawn((
            models::scene(&assets, model),
            Transform::from_scale(Vec3::ZERO),
            models::Ordered::default(),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(texture) = &looks.texture {
            rock.insert(Retexture::load(&assets, texture));
        }
        rock.id()
    });
    commands.entity(shown).insert(Burst {
        burst: LavaBurst::new(&mut dice.0),
        rock,
        trail,
        glow,
    });
    sounds.write(PlayAt {
        sound: Of::Id(particles::LAVA_BURST_SOUND),
        point,
        rate: (0.4, Roll::Share(0.2)),
    });
    flames(&mut commands, point, &looks, (&mut meshes, &mut dice.0));
}

/// Flies each burst a frame on (`0x79EEE0`, `0x5A6680`'s flag 8): its rock and its glow along the
/// arc, its trail behind; landed, a small fire on the ground there, and it goes with its trail.
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    mut bursts: Query<(Entity, &mut Burst, &Transform)>,
    mut rocks: Query<&mut Transform, Without<Burst>>,
    heights: Res<Heights>,
    (looks, mut meshes, mut dice): (Res<Looks>, ResMut<Assets<Mesh>>, ResMut<Dice>),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let ramp = LavaBurst::ramp();
    for (entity, mut shown, place) in &mut bursts {
        let origin = place.translation;
        let Burst {
            burst,
            rock,
            trail,
            glow,
        } = &mut *shown;
        if burst.update(dt, &mut dice.0) {
            let landing = origin + bolt_looks::world(burst.place());
            let ground = heights
                .standing(landing.x, landing.z, None)
                .unwrap_or(origin.y);
            flames(
                &mut commands,
                landing.with_y(ground),
                &looks,
                (&mut meshes, &mut dice.0),
            );
        }
        if burst.done() {
            if let Some(rock) = *rock {
                commands.entity(rock).despawn();
            }
            commands.entity(entity).despawn();
            continue;
        }
        let head = bolt_looks::world(burst.place());
        if let Some(mut transform) = rock.and_then(|rock| rocks.get_mut(rock).ok()) {
            let [a, b] = burst.turns;
            *transform = Transform::from_translation(origin + head)
                .with_rotation(Quat::from_rotation_z(b) * Quat::from_rotation_x(a))
                .with_scale(Vec3::splat(if burst.flies() {
                    particles::METEOR_SCALE * s
                } else {
                    0.0
                }));
        }
        let mut quads = Quads::default();
        for particle in &burst.system.particles {
            quads.push(
                bolt_looks::world(particle.place),
                particle.size * s,
                particle.angle,
                (Vec2::ZERO, 1.0),
                ramp.at(particle.life),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&*trail) {
            quads.into_mesh(&mut mesh);
        }
        let mut quads = Quads::default();
        if burst.flies() {
            quads.push(
                head,
                particles::LAVA_GLOW_HALF * s,
                0.0,
                (Vec2::ZERO, 1.0),
                particles::LAVA_GLOW_COLOUR,
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&*glow) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// Burns each small fire a frame on (`0x79F6D0`), its flames standing on their feet in their
/// orange, their frames by their age; it goes with its last flame.
fn burn(
    mut commands: Commands,
    time: Res<Time>,
    mut fires: Query<(Entity, &mut Flames)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().min(0.1);
    let grid = particles::LAVA_FLAMES_GRID;
    for (entity, mut fire) in &mut fires {
        let Flames { flames, mesh } = &mut *fire;
        flames.update(dt);
        if flames.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut quads = Quads::default();
        for particle in &flames.system.particles {
            let frame = particles::age_frame(particle.life, grid);
            let side = f32::from(grid);
            let corner = Vec2::new(f32::from(frame % grid), f32::from(frame / grid)) / side;
            quads.push_footed(
                bolt_looks::world(particle.place),
                particle.size * iso::MODEL_SCALE,
                0.0,
                (corner, 1.0 / side),
                particles::LAVA_FLAMES_COLOUR,
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&*mesh) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The lava bursts and their small fires.
pub struct LavaBurstsPlugin;

impl Plugin for LavaBurstsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Cooldown>()
            .init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (vent, fly, burn)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
