//! The glitter over the items lying on the ground and flying as they are thrown (#1382, #1477;
//! docs/re/items.md, "The glitter"): every item of a type that glitters
//! (`drops::Tables::glitters`) draws, as the objects' draw calls
//! `sdk_GLOW` (`0x408B70`), its sparks wandering about it (`opensacred_rules::particles::glitter`):
//! each a camera-facing square of `PARTICLE_SPARK04` in a hue that turns, and over it a grey one
//! of `PARTICLE_SPARK07`, both added to the frame and not tinted by the night. The sparks of
//! `DETAILLEVEL` 2, the installation's, and each item's time its object's phase, a hundred
//! milliseconds a step of its entity's index (**hypothesis**: the original's object's id).
//!
//! The mouse events that neither hide nor blink (the cursed gold, #1587) draw the same squares
//! by `0x409D00` (`opensacred_rules::particles::event_sparks`): at their cell's centre on its
//! ground (the original's height 0 over its flat ground), brought toward the camera as a stash's
//! blink is, with no phase.
//!
//! Each item's sparks lie at its place in the original's order of drawing, as its object draws
//! them (`fx::Orders::at`, #2220), in the one mesh of all of them; an event's at its cell's last
//! place, after the cell's statics and objects, as its entry of the kind `0x20` lies in the row
//! walk (`0x62CF12`, `fx::Orders::after_statics`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{GLITTER_SPARKS, SPARK_GREY, SPARK_HALF, event_sparks, glitter};
use opensacred_rules::quests::Look;

use crate::drops::{Lying, Tables};
use crate::frame::Playing;
use crate::fx::{Orders, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::stashes::Stash;

/// The textures of a spark's two squares.
const COLOURED: &str = "particle_spark04";
const GREY: &str = "particle_spark07";
/// The glitter's time runs at half the game's seconds, each object's ahead by its phase.
const RATE: f32 = 0.5;
const PHASE: f32 = 0.1 * RATE;
/// The world's units of a unit of the glitter's space, into which the draw halves the object's
/// point (`0x890670`, 0.5; seen: the sparks of this size and spread beside the original's).
const SPACE: f32 = 2.0;

/// A spark's offset `[across, along, up]` of the glitter's frame in the world: along the effects'
/// axes on the ground (`iso::fx_axes`) and up, at the glitter's scale (#2072).
fn space([across, along, up]: [f32; 3]) -> Vec3 {
    let (x, y) = iso::fx_axes();
    (x * across + y * along + Vec3::Y * up) * SPACE
}

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
    mut materials: ResMut<Assets<crate::fx::Effect>>,
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

/// Lays each glittering item's sparks out at the frame's time, and each mouse event's of the
/// sparks.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries as arguments"
)]
fn glitters(
    time: Res<Time>,
    handles: Option<Res<Meshes>>,
    tables: Option<Res<Tables>>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, Orders),
    (items, stashes): (
        Query<(Entity, &GlobalTransform, &Lying)>,
        Query<(&Stash, &Transform)>,
    ),
    settings: Option<Res<crate::settings::Settings>>,
) {
    let (Some(handles), Some(tables)) = (handles, tables) else {
        return;
    };
    // The sparks of the options' detail, 3, 5 or 8 (`0x5BB87E`).
    let sparks = GLITTER_SPARKS[usize::from(crate::settings::detail(settings.as_deref()))];
    let (mut coloured, mut grey) = (Quads::default(), Quads::default());
    let whole = (Vec2::ZERO, 1.0);
    // An item flying as it is thrown glitters too, an object drawn in the air (#1477).
    for (item, at, lying) in &items {
        if !tables.glitters(lying.dropped.item) {
            continue;
        }
        let phase = f32::from(u16::try_from(item.index_u32() % 1000).unwrap_or(0)) * PHASE;
        let point = at.translation();
        let (mut own_coloured, mut own_grey) = (Quads::default(), Quads::default());
        for spark in glitter(time.elapsed_secs() * RATE + phase, sparks) {
            // The glitter's frame, its third coordinate up, at the world's scale.
            let centre = point + space(spark.offset);
            let [r, g, b] = spark.colour;
            own_coloured.push(centre, SPARK_HALF * SPACE, 0.0, whole, [r, g, b, 1.0]);
            own_grey.push(
                centre,
                spark.half * SPACE,
                0.0,
                whole,
                [SPARK_GREY, SPARK_GREY, SPARK_GREY, 1.0],
            );
        }
        // At the item's place in the order, the mesh's origin the world's (#2220).
        let order = orders.at(point);
        for (own, all) in [(own_coloured, &mut coloured), (own_grey, &mut grey)] {
            let mut own = own;
            own.in_order(order, Vec3::ZERO);
            all.append(own);
        }
    }
    let events = event_sparks(time.elapsed_secs() * RATE, sparks);
    for (_, at) in stashes
        .iter()
        .filter(|(stash, _)| stash.look == Look::Sparks)
    {
        let point = crate::stashes::drawn_at(at);
        let (mut own_coloured, mut own_grey) = (Quads::default(), Quads::default());
        for spark in &events {
            let centre = point + space(spark.offset);
            let [r, g, b] = spark.colour;
            own_coloured.push(centre, SPARK_HALF * SPACE, 0.0, whole, [r, g, b, 1.0]);
            own_grey.push(
                centre,
                spark.half * SPACE,
                0.0,
                whole,
                [SPARK_GREY, SPARK_GREY, SPARK_GREY, 1.0],
            );
        }
        let order = orders.after_statics(at.translation);
        for (own, all) in [(own_coloured, &mut coloured), (own_grey, &mut grey)] {
            let mut own = own;
            own.in_order(order, Vec3::ZERO);
            all.append(own);
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
