//! Shadows under the characters. The original flattens each character's mesh onto the ground and
//! draws it through the stencil (docs/05 §1); until that is done, a soft dark ellipse lies under
//! each character, in the original's place for shadows, draw list 1: over the ground, the
//! liquids and the underlays of list 0 (paving, flowers), under those of list 2 and every other
//! object (docs/decisions.md).

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::frame::Step;
use crate::hero::Body;
use crate::iso;
use crate::npcs::{Npc, UpperLayer};
use crate::statics;
use crate::terrain::Heights;

/// The ellipse's radius in world units: a little more than a model's shoulders.
const RADIUS: f32 = 30.0;
/// How dark the centre is: the blending is linear, so 0.9 leaves about a third of the light.
const DARKNESS: f32 = 0.9;
/// How far back along the line of sight the shadows lie: between the underlays of lists 0 and 2,
/// as the original draws its shadows as list 1 (docs/02 §6.1), far enough from both for the
/// ellipse's own depth.
const BACK: f32 = (statics::LIST_0_BACK + statics::LIST_2_BACK) / 2.0;
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
    material: Handle<StandardMaterial>,
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
    mut materials: ResMut<Assets<StandardMaterial>>,
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
        material: materials.add(material),
    });
}

/// Gives each new character a shadow, but those of the hidden layers.
pub fn add(
    mut commands: Commands,
    blob: Res<Blob>,
    characters: Query<Entity, (NewCharacter, Without<UpperLayer>)>,
) {
    for character in &characters {
        commands.spawn((
            Mesh3d(blob.mesh.clone()),
            MeshMaterial3d(blob.material.clone()),
            Transform::IDENTITY,
            Shadow(character),
        ));
    }
}

/// Keeps each shadow under its character, on the ground, in the ground's layer; a shadow whose
/// character went goes too.
pub fn follow(
    mut commands: Commands,
    heights: Res<Heights>,
    characters: Query<&Transform, Character>,
    mut shadows: Query<(Entity, &Shadow, &mut Transform)>,
) {
    for (entity, shadow, mut transform) in &mut shadows {
        let Ok(character) = characters.get(shadow.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        let mut ground = character.translation;
        ground.y = heights.at(ground.x, ground.z).unwrap_or(ground.y);
        transform.translation = ground + iso::forward() * BACK;
        transform.scale = Vec3::splat(iso::MODEL_SCALE);
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
