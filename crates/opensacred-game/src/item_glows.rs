//! The items' own glows (#2091; docs/re/effects.md, "The arrows' elements"): what the model of
//! every item draws of its own besides its element (`0x5BEE90`, `opensacred_rules::particles`):
//! a blue glow at each of its bones `stdfx_bone01` on, the Seraphim's staff's line and the
//! lightsabers' two along their bones `weapon_fx01` and `weapon_fx02`, and the glittering wings'
//! glitter at their bones `sera03_fx0` to `3`; all added and untinted by the night, while the
//! model is held, carried or worn.

use std::collections::HashMap;

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    self, BLUE_GLOW, BLUE_GLOW_BONES, GLITTER_BONES, GLITTER_TEXTURE, GLOW_LINE_TEXTURE, GlowLine,
};

use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::npcs::Piece;
use crate::spawning::Rng;
use crate::{bolt_looks, iso};

/// The bones of the lines (15 and 16).
const LINE: [&str; 2] = ["weapon_fx01", "weapon_fx02"];

/// The glitter's dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x5BE))
    }
}

/// The glows' materials by texture and whether they are added whole, each made at the first glow
/// that takes it; none where its texture is missing.
#[derive(Resource, Default)]
struct Materials(HashMap<(&'static str, bool), Option<Handle<Effect>>>);

/// An item's model whose bones are still to be looked for, until its scene is in.
#[derive(Component)]
struct Unsought;

/// An item's own glows shown: its model, its lines and their two bones, the bones of its blue
/// glows and of its glitter, and the meshes of the three.
#[derive(Component)]
struct Shown {
    model: Entity,
    lines: &'static [GlowLine],
    line: Option<[Entity; 2]>,
    blue: Vec<Entity>,
    glitter: Vec<Entity>,
    meshes: [Handle<Mesh>; 3],
}

/// Marks each item's model put on a creature, to look for its bones.
fn mark(mut commands: Commands, pieces: Query<Entity, Added<Piece>>) {
    for piece in &pieces {
        commands.entity(piece).insert(Unsought);
    }
}

/// Looks for the bones of each marked model once its scene is in, which spawns whole: the blue
/// glows' bones its model has from the first on (`0x5BE5E0`), and by its type
/// (`drops::Tables::own_glows`) its lines' and its glitter's; a model with none shows nothing.
fn seek(
    mut commands: Commands,
    models: Query<(Entity, &Piece), With<Unsought>>,
    (children, names): (Query<&Children>, Query<&Name>),
    (tables, data, assets): (
        Option<Res<crate::drops::Tables>>,
        Option<Res<Data>>,
        Res<AssetServer>,
    ),
    (mut made, mut materials, mut meshes): (
        ResMut<Materials>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (model, piece) in &models {
        if children.get(model).is_err() {
            continue;
        }
        commands.entity(model).remove::<Unsought>();
        let own = tables
            .as_ref()
            .map(|tables| tables.own_glows(piece.item()))
            .unwrap_or_default();
        let named = |name: &str| {
            children
                .iter_descendants(model)
                .find(|&bone| names.get(bone).is_ok_and(|n| n.as_str() == name))
        };
        let blue: Vec<Entity> = BLUE_GLOW_BONES
            .iter()
            .map_while(|name| named(name))
            .collect();
        let glitter: Vec<Entity> = if own.glitters {
            GLITTER_BONES
                .iter()
                .filter_map(|name| named(name))
                .collect()
        } else {
            Vec::new()
        };
        let line = match LINE.map(named) {
            [Some(a), Some(b)] if !own.lines.is_empty() => Some([a, b]),
            _ => None,
        };
        if blue.is_empty() && glitter.is_empty() && line.is_none() {
            continue;
        }
        debug!(
            "item glows: {model} of {} glows by {} lines, {} blue and {} glittering",
            piece.item(),
            own.lines.len(),
            blue.len(),
            glitter.len()
        );
        // The lines added by their alpha (`0x40DA80`, the source's alpha), the single glows whole
        // (`0x40E2B0`, the source `ONE`).
        let textures = [
            (GLOW_LINE_TEXTURE, false),
            (BLUE_GLOW.texture, true),
            (GLITTER_TEXTURE, true),
        ];
        let parts: [Handle<Mesh>; 3] = std::array::from_fn(|_| meshes.add(empty()));
        let mut shown = commands.spawn((
            Transform::default(),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        shown.with_children(|children| {
            for (mesh, (texture, whole)) in parts.iter().zip(textures) {
                let material = made
                    .0
                    .entry((texture, whole))
                    .or_insert_with(|| {
                        let image = data
                            .as_ref()
                            .and_then(|data| particle_texture(&data.0.assets, &assets, texture));
                        if image.is_none() {
                            warn!("item glows: no texture {texture}; its glows are not drawn");
                        }
                        image.map(|image| {
                            materials.add(if whole {
                                whole_material(image)
                            } else {
                                particle_material(image, AlphaMode::Add)
                            })
                        })
                    })
                    .clone();
                if let Some(material) = material {
                    children.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material),
                        Transform::IDENTITY,
                        NoFrustumCulling,
                        RenderLayers::layer(iso::UNTINTED),
                    ));
                }
            }
        });
        shown.insert(Shown {
            model,
            lines: own.lines,
            line,
            blue,
            glitter,
            meshes: parts,
        });
    }
}

/// Draws each model's own glows a frame on at its bones (`0x5BEE90`): its lines from one bone to
/// the other (`0x40DA80`), its blue glows and its glitter, rolled anew (`0x40E2B0`); they go with
/// the model.
fn run(
    mut commands: Commands,
    mut shown: Query<(Entity, &Shown, &mut Transform)>,
    (places, holders): (Query<&GlobalTransform>, Query<&crate::models::OrderedWith>),
    (mut dice, mut meshes, orders): (ResMut<Dice>, ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let s = iso::MODEL_SCALE;
    for (entity, look, mut transform) in &mut shown {
        let Ok(model) = places.get(look.model) else {
            commands.entity(entity).despawn();
            continue;
        };
        let origin = model.translation();
        transform.translation = origin;
        // An item in a hand just behind its holder's model, one lying at its place, as the
        // original draws the system before the creature (#2188).
        let order = holders
            .get(look.model)
            .map_or_else(|_| orders.at(origin), |holder| orders.behind(holder.0));
        let at = |bone: Entity| {
            places
                .get(bone)
                .ok()
                .map(|bone| bolt_looks::models(bone.translation() - origin))
        };
        let glow = |quads: &mut Quads, point: [f32; 3], half: f32, colour: [f32; 4]| {
            quads.push(
                bolt_looks::world(point),
                half * s,
                0.0,
                (Vec2::ZERO, 1.0),
                colour,
            );
        };
        let [line_mesh, blue_mesh, glitter_mesh] = &look.meshes;
        let mut quads = Quads::default();
        if let Some([a, b]) = look.line.and_then(|[a, b]| Some([at(a)?, at(b)?])) {
            for line in look.lines {
                for point in line.points(a, b) {
                    glow(&mut quads, point, line.half, line.colour());
                }
            }
        }
        if let Some(mut mesh) = meshes.get_mut(line_mesh) {
            quads.in_order(order, origin);
            quads.into_mesh(&mut mesh);
        }
        let mut quads = Quads::default();
        for point in look.blue.iter().filter_map(|&bone| at(bone)) {
            glow(&mut quads, point, BLUE_GLOW.half, BLUE_GLOW.colour());
        }
        if let Some(mut mesh) = meshes.get_mut(blue_mesh) {
            quads.in_order(order, origin);
            quads.into_mesh(&mut mesh);
        }
        let mut quads = Quads::default();
        for point in look.glitter.iter().filter_map(|&bone| at(bone)) {
            let glitter = particles::wings_glitter(&mut dice.0);
            glow(&mut quads, point, glitter.half, glitter.colour());
        }
        if let Some(mut mesh) = meshes.get_mut(glitter_mesh) {
            quads.in_order(order, origin);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The items' own glows.
pub struct ItemGlowsPlugin;

impl Plugin for ItemGlowsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Materials>()
            .add_systems(
                Update,
                (mark, seek, run).chain().in_set(Playing).in_set(Unpaused),
            );
    }
}
