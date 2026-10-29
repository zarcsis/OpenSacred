//! The backpack's grid on the inventory's items' page (#1268; docs/re/inventory.md, §3): its 20 ×
//! 8 cells of 32 pixels at (0, 392) of the original's screen, each the image 165 (`GUI_main_04`),
//! a cell under an item tinted blue (`0x3D59AB` at the alpha `0x40`; `cInventoryEntry_render`,
//! `0x5DC430`), and the items as their 3D models, each centred in its rectangle of cells and
//! turned by its type's pose (`0x40E9C0`, `0x434270`): a camera of their own draws them,
//! orthographic, a unit of the models a pixel of the original's screen, into a texture laid over
//! the cells as the choice of the hero lays its figures (`choice::Layer`).
//!
//! What the hero picks up goes into the grid at its first fit by columns (`pickup::Pack`); while
//! the cursor holds an item over the grid, the cells under it are tinted by where it would go
//! (#1269, `held`), and the cells of an item the hero cannot use are red (#1417).

use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{RenderTarget, ScalingMode};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::ui_render::prelude::MaterialNode;
use opensacred_rules::inventory::{COLUMNS, Grid, Highlight, ROWS, held_at};

use crate::bar::{self, UiImages};
use crate::choice::Layer;
use crate::drops::{Pose, Tables};
use crate::game::AppState;
use crate::hud::Pointer;
use crate::inventory::Shown;
use crate::iso::Apart;
use crate::npcs::Retexture;
use crate::pickup::Pack;

/// The grid's top left on the original's screen, its cells' side and image, and an item's cells'
/// tint (`0x5DC430`).
const GRID: Vec2 = Vec2::new(0.0, 392.0);
pub(crate) const CELL: f32 = 32.0;
const CELL_IMAGE: u32 = 165;
const ITEM_TINT: Color = tinted(0x3D, 0x59, 0xAB);
/// The cells' tints under a held item (`0x5DC430`): where it fits, where it would swap, where it
/// would not go.
const FITS_TINT: Color = tinted(0x3D, 0x91, 0x40);
const SWAPS_TINT: Color = tinted(0x91, 0x21, 0x9E);
const REFUSED_TINT: Color = tinted(0xFF, 0x00, 0x00);
/// The render layer of the items' models, and the light their camera gives them (**estimate**).
const ITEMS: usize = 10;
pub(crate) const AMBIENT: f32 = 600.0;
pub(crate) const LIGHT: f32 = 3_000.0;

/// A cell's tint of the colour `(r, g, b)` at the alpha `0x40`.
const fn tinted(r: u8, g: u8, b: u8) -> Color {
    Color::srgba(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        64.0 / 255.0,
    )
}

/// The pointer on the original's screen from the grid's top left, in its pixels.
pub(crate) fn on_grid(
    pointer: &Pointer,
    ui_scale: &UiScale,
    windows: &Query<&Window>,
) -> Option<Vec2> {
    Some(crate::inventory::screen_point(pointer, ui_scale, windows)? - GRID)
}

/// The cell, column and row, under the point `at` from the grid's top left, where it lies on the
/// grid.
pub(crate) fn cell_under(at: Vec2) -> Option<(u8, u8)> {
    let size = Vec2::new(f32::from(COLUMNS), f32::from(ROWS)) * CELL;
    if at.x < 0.0 || at.y < 0.0 || at.x >= size.x || at.y >= size.y {
        return None;
    }
    let (column, row) = held_at((at.x, at.y), (1, 1));
    Some((u8::try_from(column).ok()?, u8::try_from(row).ok()?))
}

/// A model whose parts take its render layer and a copy of their materials that reflects no light
/// (`layer`): the grid's items and the held one (`held`).
#[derive(Component)]
pub(crate) struct Matte;

/// The grid's interface, laid out on the original's screen centred as the inventory's windows.
#[derive(Component)]
struct Root;

/// The items' camera, light and models.
#[derive(Component)]
struct Scene;

/// A cell's tint, at its column and row.
#[derive(Component)]
struct Tint((u8, u8));

/// An item's model, by its key in the pack.
#[derive(Component)]
struct Model(u32);

/// Makes the grid, its cells, the items' camera and their picture as the inventory shows, and
/// takes them away as it hides.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn show(
    mut commands: Commands,
    shown: Res<Shown>,
    olds: Query<Entity, Or<(With<Root>, With<Scene>)>>,
    (images, assets): (Res<UiImages>, Res<AssetServer>),
    mut targets: ResMut<Assets<Image>>,
    mut layers: ResMut<Assets<Layer>>,
    window: Option<Single<&Window>>,
) {
    if !shown.is_changed() {
        return;
    }
    for old in &olds {
        commands.entity(old).despawn();
    }
    if !shown.0 {
        return;
    }
    let size = Vec2::new(f32::from(COLUMNS), f32::from(ROWS)) * CELL;
    let root = commands
        .spawn((
            Root,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Px(0.0),
                margin: UiRect::left(Val::Px(-bar::SCREEN.x / 2.0)),
                width: Val::Px(bar::SCREEN.x),
                height: Val::Px(bar::SCREEN.y),
                ..default()
            },
            GlobalZIndex(-1),
            Pickable::IGNORE,
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    let grid = commands
        .spawn((bar::placed(GRID, size), Pickable::IGNORE))
        .id();
    commands.entity(root).add_child(grid);
    for column in 0..COLUMNS {
        for row in 0..ROWS {
            let at = Vec2::new(f32::from(column), f32::from(row)) * CELL;
            let (image, node) = bar::piece(&images, &assets, CELL_IMAGE, at, Vec2::ZERO);
            let cell = commands
                .spawn((image, node, Pickable::IGNORE))
                .with_child((
                    Tint((column, row)),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(grid).add_child(cell);
        }
    }
    // The items' picture, of the window's pixels, over the cells.
    let scale = window.map_or(1.0, |window| window.height() / bar::SCREEN.y);
    let pixels = (size * scale).round().as_uvec2().max(UVec2::ONE);
    let target = targets.add(Image::new_target_texture(
        pixels.x,
        pixels.y,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let picture = commands
        .spawn((
            bar::placed(Vec2::ZERO, size),
            MaterialNode(layers.add(Layer {
                figures: target.clone(),
            })),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(grid).add_child(picture);
    commands.spawn((
        Scene,
        Camera3d::default(),
        Camera {
            order: -2,
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
            brightness: AMBIENT,
            affects_lightmapped_meshes: true,
        },
        RenderLayers::layer(ITEMS),
        Apart,
        DespawnOnExit(AppState::InGame),
    ));
    commands.spawn((
        Scene,
        DirectionalLight {
            illuminance: LIGHT,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-0.5, 1.0, 1.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(ITEMS),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Where an item of `size` at the cell `at` stands, turned by its type's `pose`: its model's
/// origin at the centre of its rectangle, the grid's top left the origin, the rows going down;
/// the pose `S · Rx · Ry · Rz(−·)` then its offsets along x and z of the game's frame (Z up,
/// `0x434270`) taken into the models' (Y up, `opensacred-convert`'s `to_gltf`); a record of the
/// scale 0 the identity.
///
/// The original's frame of the models mirrors their files' y, which the converted models keep
/// (**hypothesis**, matched against its frames of the torch 5633, the arms 1218 and the staff
/// 2666): the pose is taken through that mirror, so that its turns about x and z run the other
/// way here.
pub(crate) fn placement(at: (u8, u8), size: (u8, u8), pose: Pose) -> Transform {
    let centre = Vec3::new(
        (f32::from(at.0) + f32::from(size.0) / 2.0) * CELL,
        -(f32::from(at.1) + f32::from(size.1) / 2.0) * CELL,
        0.0,
    );
    posed(centre, pose)
}

/// An item's model with its origin at `centre`, turned by its type's `pose` as [`placement`]
/// turns it (`0x40E9C0`), the grid's and a weapon set's slot's alike (`sets`).
pub(crate) fn posed(centre: Vec3, pose: Pose) -> Transform {
    if pose.scale == 0.0 {
        return Transform::from_translation(centre);
    }
    let [a, b, c] = pose.turns;
    let game = Mat4::from_translation(Vec3::new(pose.offsets[0], 0.0, pose.offsets[1]))
        * Mat4::from_rotation_z(-c)
        * Mat4::from_rotation_y(b)
        * Mat4::from_rotation_x(a)
        * Mat4::from_scale(Vec3::splat(pose.scale));
    let mirror = Mat4::from_scale(Vec3::new(1.0, -1.0, 1.0));
    let turn = Mat4::from_rotation_x(-FRAC_PI_2);
    Transform::from_translation(centre)
        * Transform::from_matrix(turn * mirror * game * mirror * turn.inverse())
}

/// Tints the cells while the inventory shows: under an item held over the grid by where it
/// would go, green, purple or red (`0x5DC430`); else under an item blue, or red where the hero
/// may not use it (`0x54ABE0`, #1417).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn tint(
    shown: Res<Shown>,
    pack: Res<Pack>,
    (pointer, ui_scale): (Res<Pointer>, Res<UiScale>),
    windows: Query<&Window>,
    (hero, experience, rules, tables): (
        Option<Res<crate::hero::Hero>>,
        Res<crate::fight::Experience>,
        Res<crate::fight::Rules>,
        Option<Res<Tables>>,
    ),
    mut tints: Query<(&Tint, &mut BackgroundColor)>,
) {
    if !shown.0 {
        return;
    }
    let held = pack.held.and_then(|held| {
        let at = on_grid(&pointer, &ui_scale, &windows)?;
        cell_under(at)?;
        let cell = held_at((at.x, at.y), held.size);
        Some((cell, held.size, pack.grid.highlight(cell, held.size)))
    });
    let refused = |key: u32| {
        let (Some(hero), Some(tables)) = (hero.as_deref(), tables.as_deref()) else {
            return false;
        };
        pack.items
            .iter()
            .find(|(k, _)| *k == key)
            .is_some_and(|(_, dropped)| {
                let asks = tables.asks(dropped, &rules);
                !opensacred_rules::equipment::may_use(hero.creature, experience.0.level, &asks)
            })
    };
    for (tint, mut colour) in &mut tints {
        let wanted = match held {
            Some((at, size, highlight)) if Grid::held_over(tint.0, at, size) => match highlight {
                Highlight::Fits => FITS_TINT,
                Highlight::Swaps => SWAPS_TINT,
                Highlight::Refused => REFUSED_TINT,
            },
            _ => match pack.grid.at(tint.0) {
                Some(placed) if refused(placed.key) => REFUSED_TINT,
                Some(_) => ITEM_TINT,
                None => Color::NONE,
            },
        };
        colour.set_if_neq(BackgroundColor(wanted));
    }
}

/// Lays the items' models out as the pack holds them, while the inventory shows: a model for each
/// item, made as it comes, taken away as it goes, and moved where the item moves (the sort,
/// #1321).
fn lay_out(
    mut commands: Commands,
    shown: Res<Shown>,
    pack: Res<Pack>,
    (tables, assets): (Option<Res<Tables>>, Res<AssetServer>),
    mut models: Query<(Entity, &Model, &mut Transform)>,
    added: Query<(), Added<Tint>>,
) {
    if !shown.0 || !(pack.is_changed() || shown.is_changed() || !added.is_empty()) {
        return;
    }
    let Some(tables) = tables else {
        return;
    };
    for (entity, model, _) in &models {
        if !pack.grid.items.iter().any(|item| item.key == model.0) {
            commands.entity(entity).despawn();
        }
    }
    for placed in &pack.grid.items {
        let Some(item) = pack
            .items
            .iter()
            .find(|(key, _)| *key == placed.key)
            .map(|(_, dropped)| dropped.item)
        else {
            continue;
        };
        let Some((model, texture, pose)) = tables.look(item) else {
            continue;
        };
        let at = placement(placed.at, placed.size, pose);
        if let Some((.., mut transform)) = models
            .iter_mut()
            .find(|(_, model, _)| model.0 == placed.key)
        {
            if *transform != at {
                *transform = at;
            }
            continue;
        }
        let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(model.to_owned()));
        let mut entity = commands.spawn((
            Scene,
            Model(placed.key),
            Matte,
            WorldAssetRoot(scene),
            at,
            RenderLayers::layer(ITEMS),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(texture) = texture {
            entity.insert(Retexture::load(&assets, texture));
        }
    }
}

/// Puts each part of the items' models into their camera's layer as their scenes come in, and
/// gives it a copy of its material that reflects no light (**estimate**): the original lights an
/// item by its colour alone, where the ambient light's reflection of 4% would lift its darks to a
/// grey.
pub(crate) fn layer(
    mut commands: Commands,
    models: Query<(Entity, &RenderLayers), With<Matte>>,
    children: Query<&Children>,
    layered: Query<(), With<RenderLayers>>,
    mut parts: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut matte: Local<HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>>,
) {
    for (model, layers) in &models {
        for part in children.iter_descendants(model) {
            if !layered.contains(part) {
                commands.entity(part).insert(layers.clone());
            }
            let Ok(mut material) = parts.get_mut(part) else {
                continue;
            };
            let id = material.0.id();
            if matte.values().any(|copy| copy.id() == id) {
                continue;
            }
            if let Some(copy) = matte.get(&id) {
                material.0 = copy.clone();
            } else if let Some(source) = materials.get(id).cloned() {
                let copy = materials.add(StandardMaterial {
                    reflectance: 0.0,
                    ..source
                });
                matte.insert(id, copy.clone());
                material.0 = copy;
            }
        }
    }
}

/// The backpack's grid on the inventory's items' page.
pub struct BackpackPlugin;

impl Plugin for BackpackPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (show, lay_out, tint, layer)
                .chain()
                .after(crate::inventory::Opened)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_item_stands_at_its_rectangles_centre_turned_into_the_models_frame() {
        // A sword of 1 × 3 at the column 2, unturned: its origin at (80, −48).
        let plain = Pose {
            scale: 1.0,
            ..Pose::default()
        };
        let at = placement((2, 0), (1, 3), plain);
        assert!((at.translation - Vec3::new(80.0, -48.0, 0.0)).length() < 1e-4);
        assert!(at.rotation.angle_between(Quat::IDENTITY) < 1e-4);
        // The game's offset along its z, its up, goes up the models' y.
        let raised = placement(
            (0, 0),
            (1, 1),
            Pose {
                offsets: [0.0, 5.0],
                ..plain
            },
        );
        assert!((raised.translation - Vec3::new(16.0, -11.0, 0.0)).length() < 1e-4);
        // A turn about the game's up turns about the models' y.
        let turned = placement(
            (0, 0),
            (1, 1),
            Pose {
                turns: [0.0, 0.0, FRAC_PI_2],
                ..plain
            },
        );
        let forward = turned.rotation * Vec3::X;
        assert!(forward.y.abs() < 1e-4 && forward.z.abs() > 0.99);
        // The torch's turn about the game's x stands its shaft, along its file's y (the models'
        // −z), upright through the mirror.
        let torch = placement(
            (0, 0),
            (1, 2),
            Pose {
                turns: [-1.52, 0.0, 0.0],
                ..plain
            },
        );
        assert!((torch.rotation * Vec3::NEG_Z).y > 0.99);
        // A record of the scale 0 leaves the model as it is.
        let none = placement((0, 0), (1, 1), Pose::default());
        assert_eq!(none.scale, Vec3::ONE);
    }
}
