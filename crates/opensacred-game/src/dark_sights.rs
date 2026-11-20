//! The Dark Elf's sights of the hit sparks' class that are no hits (864, #2923;
//! docs/re/effects.md, "The hit sparks"): the magic circle that turns under each creature the
//! viper 1042 holds, for its seconds (`making`); the symbol of a turned spell's largest kind under
//! each creature the spellproof 1046 strikes, fading over 1.33 s (`spells`); and the boons 1047
//! and 1048's crackle of red or yellow lightning at each hand, for the boon's seconds, in a glow
//! (`making`); all of them lighting the night about them (`opensacred_rules::particles::
//! DarkSight`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    DARK_GLOW, DARK_LIGHTNING, DARK_LIGHTNING_GRID, DARK_SIGHT_HALF, DARK_SIGHT_LIGHT, DarkSight,
    DarkSightKind,
};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;
use crate::terror_sprinters::flat;

/// A sight asked for: its bearer, its kind, its seconds, the bone it sits at (its root where
/// none), and whether it follows its bearer rather than staying where it is first placed.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Sight {
    pub bearer: Entity,
    pub kind: DarkSightKind,
    pub seconds: f32,
    pub bone: Option<&'static str>,
    pub follows: bool,
}

/// A sight's entity, to spawn.
pub(crate) fn sight(sight: Sight) -> impl Bundle {
    (
        sight,
        Transform::default(),
        Visibility::default(),
        crate::daynight::Light(DARK_SIGHT_LIGHT),
        DespawnOnExit(AppState::InGame),
    )
}

/// A sight shown: its state, its place once placed, and its meshes, the flat quad's, the bolts'
/// and the glow's.
#[derive(Component)]
struct Shown {
    sight: DarkSight,
    placed: Option<Vec3>,
    meshes: [Handle<Mesh>; 3],
}

/// The sights' materials by texture, made as first asked; none where a texture is missing.
#[derive(Resource, Default)]
struct Materials(Vec<(&'static str, Option<Handle<Effect>>)>);

impl Materials {
    fn of(
        &mut self,
        texture: &'static str,
        make: impl Fn(Handle<Image>) -> Effect,
        (data, assets, effects): (Option<&Data>, &AssetServer, &mut Assets<Effect>),
    ) -> Option<Handle<Effect>> {
        if let Some((_, material)) = self.0.iter().find(|(name, _)| *name == texture) {
            return material.clone();
        }
        let image = data.and_then(|data| particle_texture(&data.0.assets, assets, texture));
        if image.is_none() {
            warn!("dark sights: no texture {texture}; it is not drawn");
        }
        let material = image.map(|image| effects.add(make(image)));
        self.0.push((texture, material.clone()));
        material
    }
}

/// The sights' dice (`rand()`; **estimate**: any seed).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x864))
    }
}

/// Gives each new sight its state and its meshes (`0x794CE0`).
fn make(
    mut commands: Commands,
    sights: Query<(Entity, &Sight), Without<Shown>>,
    (data, assets, mut made): (Option<Res<Data>>, Res<AssetServer>, ResMut<Materials>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, ask) in &sights {
        let state = DarkSight::new(ask.kind, ask.seconds);
        let flat = state.flat().and_then(|flat| {
            let make: fn(Handle<Image>) -> Effect = if flat.whole {
                whole_material
            } else {
                |image| particle_material(image, AlphaMode::Blend)
            };
            made.of(flat.texture, make, (data.as_deref(), &assets, &mut effects))
        });
        let crackles = matches!(ask.kind, DarkSightKind::Crackle { .. });
        let (bolts, glow) = if crackles {
            (
                made.of(
                    DARK_LIGHTNING,
                    |image| particle_material(image, AlphaMode::Add),
                    (data.as_deref(), &assets, &mut effects),
                ),
                made.of(
                    DARK_GLOW,
                    whole_material,
                    (data.as_deref(), &assets, &mut effects),
                ),
            )
        } else {
            (None, None)
        };
        let parts: [Handle<Mesh>; 3] = std::array::from_fn(|_| meshes.add(empty()));
        let mut shown = commands.entity(entity);
        for (mesh, material) in parts.iter().zip([flat, bolts, glow]) {
            if let Some(material) = material {
                shown.with_child((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                ));
            }
        }
        shown.insert(Shown {
            sight: state,
            placed: None,
            meshes: parts,
        });
        debug!(
            "dark sights: {:?} on {} for {} s",
            ask.kind, ask.bearer, ask.seconds
        );
    }
}

/// Runs and draws each sight a frame on at its bearer's bone, or where it was placed
/// (`0x7946A0`, `0x794820`); it goes as it ends or with its bearer.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &Sight, &mut Shown, &mut Transform)>,
    places: Query<&GlobalTransform>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut dice, mut meshes, orders): (ResMut<Dice>, ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, ask, mut look, mut transform) in &mut shown {
        let look = &mut *look;
        let Ok(bearer) = places.get(ask.bearer) else {
            commands.entity(entity).despawn();
            continue;
        };
        if !look.sight.update(dt, &mut dice.0) {
            commands.entity(entity).despawn();
            continue;
        }
        let here = ask
            .bone
            .and_then(|name| crate::making::bone(ask.bearer, &[name], &children, (&bones, &worn)))
            .unwrap_or_else(|| bearer.translation());
        let origin = match look.placed {
            Some(placed) if !ask.follows => placed,
            _ => here,
        };
        look.placed = Some(origin);
        transform.translation = origin;
        let order = if ask.follows {
            orders.behind(ask.bearer)
        } else {
            orders.at(origin)
        };
        let [flat_mesh, bolts_mesh, glow_mesh] = &look.meshes;
        let mut quads = Quads::default();
        if let Some(flat_quad) = look.sight.flat() {
            quads.push_corners(
                flat(DARK_SIGHT_HALF, flat_quad.turn),
                [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                flat_quad.colour,
            );
        }
        if let Some(mut mesh) = meshes.get_mut(flat_mesh) {
            quads.in_order(order, origin);
            quads.into_mesh(&mut mesh);
        }
        let mut bolts = Quads::default();
        let mut glow = Quads::default();
        if let DarkSightKind::Crackle { red } = ask.kind {
            let grid = f32::from(DARK_LIGHTNING_GRID);
            for bolt in &look.sight.system.particles {
                // Its frame of the grid by its age (`⌊(255 − life)·G²/256⌋`).
                let frame = ((255.0 - bolt.life) * grid * grid / 256.0)
                    .clamp(0.0, grid * grid - 1.0)
                    .floor();
                let corner = Vec2::new(frame % grid, (frame / grid).floor()) / grid;
                bolts.push_footed(
                    world(bolt.place),
                    bolt.size * s,
                    bolt.angle,
                    (corner, 1.0 / grid),
                    DarkSight::crackle_colour(red, bolt.life),
                );
            }
            let roll = f32::from(u16::try_from(dice.0.below(32_768)).unwrap_or(0)) / 32_768.0;
            if let Some((half, colour)) = look.sight.glow(roll) {
                glow.push_facing(Vec3::ZERO, half * s, [colour; 4]);
            }
        }
        for (mut quads, mesh) in [bolts, glow].into_iter().zip([bolts_mesh, glow_mesh]) {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.in_order(order, origin);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The Dark Elf's sights.
pub struct DarkSightsPlugin;

impl Plugin for DarkSightsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Materials>()
            .add_systems(
                Update,
                (make, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
