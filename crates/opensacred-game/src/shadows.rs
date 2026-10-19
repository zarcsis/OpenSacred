//! Shadows under the characters. The original flattens each character's mesh onto the ground and
//! draws it through the stencil (docs/05 §1), with the model, in the model's place in the draw
//! lists (`cGranny::renderShadow`, #101). Until that is done, a soft dark ellipse lies under each
//! character, drawn as a part of its model that lies on the ground (`models::ModelMaterial`,
//! tagged with the character's slot of the anchors): over what the original draws before the
//! model (the ground, the liquids, the underlays, the floor and the stairs the character stands
//! on), under the model itself and what it draws after (docs/decisions/2026-10-17-shadows.md).

use bevy::asset::RenderAssetUsages;
use bevy::mesh::MeshTag;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::frame::Step;
use crate::hero::Body;
use crate::iso;
use crate::models::{self, Anchors, ModelMaterial, Ordered};
use crate::npcs::Npc;
use crate::terrain::Heights;

/// The ellipse's radius in world units: a little more than a model's shoulders.
const RADIUS: f32 = 30.0;
/// How dark the centre is: the blending is linear, so 0.9 leaves about a third of the light.
const DARKNESS: f32 = 0.9;
/// How long the ellipse is across the line of sight to its depth along it.
const FLAT: f32 = 0.7;
/// The part of the radius as dark as the centre.
const SOLID: f32 = 0.4;
/// The side of the fade's image.
const TEXTURE: u32 = 64;

/// A character that just came: the hero or an NPC.
type NewCharacter = Or<(Added<Body>, Added<Npc>)>;
/// A character, not its shadow.
type Character = (Or<(With<Body>, With<Npc>)>, Without<Shadow>);

/// The shadow of a character.
#[derive(Component)]
pub struct Shadow(Entity);

/// The shared ellipse: a quad on the ground plane with a dark centre fading to a clear rim.
#[derive(Resource)]
pub struct Blob {
    mesh: Handle<Mesh>,
    material: Handle<ModelMaterial>,
}

/// The fade of the ellipse, as an image: black, its alpha `DARKNESS` up to `SOLID` of the radius,
/// then easing to clear at the rim.
fn fade() -> Image {
    let half = TEXTURE as f32 / 2.0;
    let mut pixels = Vec::new();
    for y in 0..TEXTURE {
        for x in 0..TEXTURE {
            let (dx, dy) = (
                (x as f32 + 0.5 - half) / half,
                (y as f32 + 0.5 - half) / half,
            );
            let t = ((dx.hypot(dy) - SOLID) / (1.0 - SOLID)).clamp(0.0, 1.0);
            let alpha = DARKNESS * (1.0 - t * t * (3.0 - 2.0 * t));
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "alpha is in 0..=1"
            )]
            pixels.extend([0, 0, 0, (alpha * 255.0).round() as u8]);
        }
    }
    Image::new(
        Extent3d {
            width: TEXTURE,
            height: TEXTURE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    )
}

pub fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ModelMaterial>>,
    anchors: Res<Anchors>,
) {
    let mesh = Plane3d::new(Vec3::Y, Vec2::new(RADIUS, RADIUS * FLAT));
    let material = StandardMaterial {
        base_color_texture: Some(images.add(fade())),
        unlit: true,
        cull_mode: None,
        alpha_mode: AlphaMode::Blend,
        ..default()
    };
    commands.insert_resource(Blob {
        mesh: meshes.add(mesh),
        material: materials.add(models::in_order(material, &anchors)),
    });
}

/// Gives each new character a shadow at its place in the order, hidden until it follows the
/// character.
pub fn add(
    mut commands: Commands,
    blob: Res<Blob>,
    characters: Query<(Entity, &Ordered), NewCharacter>,
) {
    for (character, ordered) in &characters {
        let Some(slot) = ordered.slot() else {
            continue;
        };
        commands.spawn((
            Mesh3d(blob.mesh.clone()),
            MeshMaterial3d(blob.material.clone()),
            MeshTag::new(slot),
            Transform::IDENTITY,
            Visibility::Hidden,
            Shadow(character),
        ));
    }
}

/// Keeps each shadow under its character, on the ground, and shown while its character is (an
/// NPC of a building's inside only while the building shows it); a shadow whose character went
/// goes too.
pub fn follow(
    mut commands: Commands,
    heights: Res<Heights>,
    characters: Query<(&Transform, Option<&Visibility>), Character>,
    mut shadows: Query<(Entity, &Shadow, &mut Transform, &mut Visibility)>,
) {
    for (entity, shadow, mut transform, mut visibility) in &mut shadows {
        let Ok((character, shown)) = characters.get(shadow.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        let mut ground = character.translation;
        ground.y = heights.at(ground.x, ground.z).unwrap_or(ground.y);
        transform.translation = ground;
        transform.scale = Vec3::splat(iso::MODEL_SCALE);
        visibility.set_if_neq(if shown == Some(&Visibility::Hidden) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
}

/// The characters' shadows.
pub struct ShadowsPlugin;

impl Plugin for ShadowsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, (add, follow).chain().in_set(Step::Shadows));
    }
}
