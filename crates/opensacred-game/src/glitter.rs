//! The glitter over the items lying on the ground (#1382; docs/re/items.md, "The glitter"): every
//! item of a type that glitters (`drops::Tables::glitters`) draws, as the objects' draw calls
//! `sdk_GLOW` (`0x408B70`), its sparks wandering about it (`opensacred_rules::particles::glitter`):
//! each a camera-facing square of `PARTICLE_SPARK04` in a hue that turns, and over it a grey one
//! of `PARTICLE_SPARK07`, both added to the frame and not tinted by the night. The sparks of
//! `DETAILLEVEL` 2, the installation's, and each item's time its object's phase, a hundred
//! milliseconds a step of its entity's index (**hypothesis**: the original's object's id).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{GLITTER_SPARKS, SPARK_GREY, SPARK_HALF, glitter};

use crate::drops::{Lying, Tables};
use crate::frame::Playing;
use crate::fx::{Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;

/// The textures of a spark's two squares.
const COLOURED: &str = "particle_spark04";
const GREY: &str = "particle_spark07";
/// The sparks of the installation's `DETAILLEVEL`, 2.
const SPARKS: u8 = GLITTER_SPARKS[2];
/// The glitter's time runs at half the game's seconds, each object's ahead by its phase.
const RATE: f32 = 0.5;
const PHASE: f32 = 0.1 * RATE;
/// The world's units of a unit of the glitter's space, into which the draw halves the object's
/// point (`0x890670`, 0.5; seen: the sparks of this size and spread beside the original's).
const SPACE: f32 = 2.0;

/// The meshes of the sparks' coloured squares and of their grey ones.
#[derive(Resource)]
struct Meshes([Handle<Mesh>; 2]);

/// Makes the glitter's two meshes and their materials as a game starts; none where a texture is
/// missing.
fn begin(
    mut commands: Commands,
    data: Option<Res<Data>>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(data) = data else {
        return;
    };
    let handles = [COLOURED, GREY].map(|name| {
        let mesh = meshes.add(empty());
        match particle_texture(&data.0.assets, &assets, name) {
            Some(image) => {
                commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(materials.add(particle_material(image, AlphaMode::Add))),
                    Transform::default(),
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                    DespawnOnExit(AppState::InGame),
                ));
            }
            None => warn!("glitter: no texture {name}; the items do not glitter"),
        }
        mesh
    });
    commands.insert_resource(Meshes(handles));
}

/// Lays each glittering item's sparks out at the frame's time.
fn glitters(
    time: Res<Time>,
    handles: Option<Res<Meshes>>,
    tables: Option<Res<Tables>>,
    mut meshes: ResMut<Assets<Mesh>>,
    items: Query<(Entity, &GlobalTransform, &Lying)>,
) {
    let (Some(handles), Some(tables)) = (handles, tables) else {
        return;
    };
    let (mut coloured, mut grey) = (Quads::default(), Quads::default());
    let whole = (Vec2::ZERO, 1.0);
    for (item, at, lying) in &items {
        if !lying.resting || !tables.glitters(lying.dropped.item) {
            continue;
        }
        let phase = f32::from(u16::try_from(item.index_u32() % 1000).unwrap_or(0)) * PHASE;
        let point = at.translation();
        for spark in glitter(time.elapsed_secs() * RATE + phase, SPARKS) {
            // The glitter's frame, its third coordinate up, at the world's scale.
            let [across, along, up] = spark.offset;
            let centre = point + Vec3::new(across, up, along) * SPACE;
            let [r, g, b] = spark.colour;
            coloured.push(centre, SPARK_HALF * SPACE, 0.0, whole, [r, g, b, 1.0]);
            grey.push(
                centre,
                spark.half * SPACE,
                0.0,
                whole,
                [SPARK_GREY, SPARK_GREY, SPARK_GREY, 1.0],
            );
        }
    }
    for (handle, quads) in handles.0.iter().zip([coloured, grey]) {
        if let Some(mut mesh) = meshes.get_mut(handle) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The items' glitter.
pub struct GlitterPlugin;

impl Plugin for GlitterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), begin)
            .add_systems(Update, glitters.in_set(Playing));
    }
}
