//! The taskbar's five weapon sets (#1347; docs/re/inventory.md, §5, and docs/re/interface.md,
//! "The bar"): two halves each, the left hand's and the right's (the inventory's entry from
//! `+0xC18`, `0x5D8700`), a new game's first holding the template's items in the hero's hands as
//! the templates keep them (docs/02 §9), the first selected.
//!
//! Each slot that the hero's level opens draws its set's two items as their 3D models in its orb
//! (`0x6E29E0`), over its ring: each model's origin at the slot's corner plus its type's offsets
//! (`0x434480`, `drops::Tables::in_slot`), 8 pixels apart either way where both halves hold a
//! weapon (`0x427A40`), at the scale 1.0 and turned by its backpack's pose, as the grid draws its
//! items (`0x40E9C0`, `backpack`); a camera of their own draws them into a picture over the bar's
//! left slots.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{RenderTarget, ScalingMode};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::ui_render::prelude::MaterialNode;
use opensacred_data::heroes::Worn;
use opensacred_rules::drops::{Dropped, Kind};

use crate::backpack::{self, Matte};
use crate::bar::{self, BarRoot};
use crate::choice::Layer;
use crate::drops::Tables;
use crate::fight;
use crate::game::AppState;
use crate::iso::Apart;
use crate::npcs::Retexture;

/// The weapon sets, and the halves of each.
pub const SETS: usize = 5;
const HALVES: usize = 2;
/// The part of the original's screen that the sets' models are drawn in: the left slots, and
/// room above and below them for a long weapon, which overhangs its orb (seen: the Seraphim's
/// sword).
const AREA: Rect = Rect {
    min: Vec2::new(0.0, 640.0),
    max: Vec2::new(400.0, 768.0),
};
/// The render layer of the sets' models.
const MODELS: usize = 13;
/// How far apart the two halves' weapons are drawn (`0x6E29E0`).
const APART: f32 = 8.0;
/// How far toward the camera the right hand's model lies and away from it the left's, so that
/// the right's is drawn over the left's as the original draws them, one after the other (seen:
/// the Vampiress's sword over her shield).
const DEPTH: f32 = 500.0;
/// The weapons' category (`items.pak +0x2E`, `0x427A40`).
const WEAPON: u8 = 5;

/// The template's items in the hero's hands at a new game's start, its left hand's and its
/// right's, which `game` reads with the template.
#[derive(Resource, Default, Clone, Debug)]
pub struct Starting(pub [Option<Worn>; HALVES]);

/// The hero's weapon sets: each set's two halves, the left hand's and the right's; the first
/// selected (its choice #1348's).
#[derive(Resource, Default, Clone, Debug)]
pub struct WeaponSets {
    pub halves: [[Option<Dropped>; HALVES]; SETS],
}

/// A set's half's model.
#[derive(Component)]
struct Model;

/// The sets' camera, light and picture.
#[derive(Component)]
struct Scene;

/// The hero's weapon sets at each game's start: the template's items in its hands in the first
/// set, of their records' values, the first selected.
fn start(mut commands: Commands, starting: Option<Res<Starting>>, tables: Option<Res<Tables>>) {
    let mut sets = WeaponSets::default();
    for (half, worn) in starting
        .iter()
        .flat_map(|starting| starting.0.iter().enumerate())
    {
        let Some(worn) = worn else {
            continue;
        };
        let kind = tables
            .as_deref()
            .and_then(|tables| tables.catalogue().kind(worn.item))
            .unwrap_or(Kind::Weapon);
        sets.halves[0][half] = Some(Dropped {
            kind,
            item: worn.item,
            level: 0,
            key: 16,
            quality: 0,
            amount: 1,
            values: Some(fight::ItemValues::of_record(&worn.words)),
        });
    }
    commands.insert_resource(sets);
}

/// Makes the sets' camera and their picture over the bar's left slots once the bar is made.
fn show(
    mut commands: Commands,
    bars: Query<Entity, Added<BarRoot>>,
    olds: Query<Entity, With<Scene>>,
    mut targets: ResMut<Assets<Image>>,
    mut layers: ResMut<Assets<Layer>>,
    window: Option<Single<&Window>>,
) {
    let Ok(bar) = bars.single() else {
        return;
    };
    for old in &olds {
        commands.entity(old).despawn();
    }
    let size = AREA.size();
    let scale = window.map_or(1.0, |window| window.height() / bar::SCREEN.y);
    let pixels = (size * scale).round().as_uvec2().max(UVec2::ONE);
    let target = targets.add(Image::new_target_texture(
        pixels.x,
        pixels.y,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    // Over the slots' orbs and rings, as the models are drawn after them.
    let picture = commands
        .spawn((
            Scene,
            bar::placed(AREA.min - bar::BAR.min, size),
            MaterialNode(layers.add(Layer {
                figures: target.clone(),
            })),
            ZIndex(1),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(bar).add_child(picture);
    commands.spawn((
        Scene,
        Camera3d::default(),
        Camera {
            order: -3,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        RenderTarget::from(target),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: size.x,
                height: size.y,
            },
            near: -2_000.0,
            far: 2_000.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(size.x / 2.0, -size.y / 2.0, 0.0),
        Tonemapping::None,
        DebandDither::Disabled,
        AmbientLight {
            color: Color::WHITE,
            brightness: backpack::AMBIENT,
            affects_lightmapped_meshes: true,
        },
        RenderLayers::layer(MODELS),
        Apart,
        DespawnOnExit(AppState::InGame),
    ));
    commands.spawn((
        Scene,
        DirectionalLight {
            illuminance: backpack::LIGHT,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-0.5, 1.0, 1.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(MODELS),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Where a set's half's model lies in the picture: its slot's corner plus its type's offsets, 8
/// pixels to the right for the left hand and to the left for the right where both hold a weapon;
/// the right hand's before the left's.
fn origin(corner: Vec2, offset: Vec2, half: usize, both_weapons: bool) -> Vec3 {
    let apart = match (both_weapons, half) {
        (false, _) => 0.0,
        (true, 0) => APART,
        (true, _) => -APART,
    };
    let at = corner + offset + Vec2::new(apart, 0.0) - AREA.min;
    Vec3::new(at.x, -at.y, if half == 0 { -DEPTH } else { DEPTH })
}

/// Lays the shown sets' models out in their slots as the sets or the slots shown change.
fn lay_out(
    mut commands: Commands,
    sets: Option<Res<WeaponSets>>,
    shown: Option<Res<bar::Shown>>,
    (tables, assets): (Option<Res<Tables>>, Res<AssetServer>),
    models: Query<Entity, With<Model>>,
) {
    let (Some(sets), Some(shown), Some(tables)) = (sets, shown, tables) else {
        return;
    };
    if !(sets.is_changed() || shown.is_changed()) {
        return;
    }
    for model in &models {
        commands.entity(model).despawn();
    }
    for (set, halves) in sets.halves.iter().enumerate().take(shown.0) {
        let both_weapons = halves
            .iter()
            .all(|half| half.is_some_and(|item| tables.category(item.item) == WEAPON));
        let corner = bar::weapon_slot(set, shown.0);
        for (half, item) in halves.iter().enumerate() {
            let Some(item) = item else {
                continue;
            };
            let Some((model, texture, pose)) = tables.look(item.item) else {
                continue;
            };
            let at = origin(corner, tables.in_slot(item.item), half, both_weapons);
            let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(model.to_owned()));
            let mut entity = commands.spawn((
                Model,
                Matte,
                WorldAssetRoot(scene),
                backpack::posed(at, pose),
                RenderLayers::layer(MODELS),
                DespawnOnExit(AppState::InGame),
            ));
            if let Some(texture) = texture {
                entity.insert(Retexture::load(&assets, texture));
            }
        }
    }
}

/// The taskbar's weapon sets.
pub struct SetsPlugin;

impl Plugin for SetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), start)
            .add_systems(
                Update,
                (show, lay_out).chain().run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_halfs_model_lies_at_its_slots_corner_and_offsets_apart_from_the_other_weapon() {
        let corner = bar::weapon_slot(0, 1);
        assert_eq!(corner, Vec2::new(328.0, 691.0));
        let offset = Vec2::new(18.0, 53.0);
        // Alone: the corner plus the offsets, in the picture of the area from its top left, the
        // right hand's before the left's.
        assert_eq!(
            origin(corner, offset, 1, false),
            Vec3::new(346.0, -104.0, DEPTH)
        );
        assert_eq!(origin(corner, offset, 0, false).z, -DEPTH);
        // Two weapons: the left hand's 8 to the right, the right hand's 8 to the left.
        assert_eq!(origin(corner, offset, 0, true).x, 354.0);
        assert_eq!(origin(corner, offset, 1, true).x, 338.0);
    }
}
