//! The cursor's item (#1269; docs/re/inventory.md, §3 "The clicks on the items' page" and §6):
//! the left button on the grid takes the item under the cursor onto it with no sound, puts the
//! held item where it fits with its category's sound (`(9, 1)`), or in place of the one item under
//! it, which goes onto the cursor (`0x6C0150`); an item picked up while the items' page shows goes
//! onto the empty cursor (`pickup`). The held item is its 3D model centred on the cursor (seen in
//! the original with a potion), drawn over everything by a camera of its own as the grid's items
//! are drawn (`backpack`), and the system's cursor hides while it is held: the item stays the
//! cursor (`0x611E70`); the cell it goes to is the one under the cursor less its offset
//! (`0x655810`). A left click on the world drops it beside the hero, on the cell toward the
//! click that may be walked on (`cEngine::dropItem`, `0x611620`), and does nothing else.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{RenderTarget, ScalingMode};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::ui_render::prelude::MaterialNode;
use bevy::window::{CursorOptions, PrimaryWindow};
use opensacred_rules::inventory::{Put, drop_cell, held_at};

use crate::backpack::{self, Matte, cell_under, on_grid};
use crate::buildings::Buildings;
use crate::choice::Layer;
use crate::drops::{self, Dice, Pose, Tables};
use crate::frame::{Control, Unpaused};
use crate::game::AppState;
use crate::hero::Body;
use crate::hud::Pointer;
use crate::inventory::Shown;
use crate::iso::Apart;
use crate::npcs::Retexture;
use crate::pickup::Pack;
use crate::terrain::Heights;
use crate::{attack, hero, paths, pick, sfx};

/// The render layer of the held item's model, and how high its picture lies over the interface.
const HELD: usize = 11;
const OVER_ALL: i32 = 1_000;

/// The held item's camera, light and picture.
#[derive(Component)]
struct View;

/// The held item's model: its key in the pack, its size and its pose.
#[derive(Component)]
struct Model {
    key: u32,
    size: (u8, u8),
    pose: Pose,
}

/// The sound of an item of the category `category` put into the grid or a slot (`(9, 1)` and
/// `(9, 5)`, docs/re/sound.md, "Items, gold and levels"), and its rate.
pub(crate) fn put_sound(category: u8) -> (u32, (f32, sfx::Roll)) {
    let id = match category {
        6 | 18 | 19 | 21 | 23 | 24 => 7116,
        8 => 7007,
        9 => 7019,
        13 => 7115,
        14 => 7114,
        16 => 7107,
        17 => 7113,
        20 | 28 => 7020,
        _ => return (7005, (0.45, sfx::Roll::Share(0.1))),
    };
    (id, (0.4, sfx::Roll::Share(0.2)))
}

/// The backpack's grid's press, which a merchant's sale by Shift goes before (#1619).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Click;

/// A left click on the grid while the inventory shows (`0x6C0150`): with the cursor empty, the
/// item under it onto the cursor; with an item, put where it fits, with its sound, or in place of
/// the one item under it; else nothing, and nothing while the purchase window asks.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn click(
    (shown, page): (Res<Shown>, Res<crate::inventory::Page>),
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale): (Res<Pointer>, Res<UiScale>),
    windows: Query<&Window>,
    mut pack: ResMut<Pack>,
    tables: Option<Res<Tables>>,
    mut sounds: MessageWriter<sfx::Play>,
    asking: Option<Res<crate::purchase::Asking>>,
) {
    if !shown.0 || asking.is_some() || !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(at) = on_grid(&pointer, &ui_scale, &windows, &page) else {
        return;
    };
    let Some(cell) = cell_under(at) else {
        return;
    };
    let Some(held) = pack.held else {
        if pack.lift(cell) {
            debug!("held: the item at {cell:?} onto the cursor");
        }
        return;
    };
    let put = pack.put_held(held_at((at.x, at.y), held.size));
    debug!("held: {} put at {cell:?}: {put:?}", held.dropped.item);
    if put == Put::Placed {
        let category = tables.map_or(0, |tables| tables.category(held.dropped.item));
        let (id, rate) = put_sound(category);
        sounds.write(sfx::Play {
            sound: sfx::Of::Id(id),
            at: None,
            rate,
        });
    }
}

/// Makes the held item's camera, picture and model as an item comes onto the cursor, and takes
/// them away as it leaves.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn show(
    mut commands: Commands,
    pack: Res<Pack>,
    (tables, assets): (Option<Res<Tables>>, Res<AssetServer>),
    views: Query<Entity, Or<(With<View>, With<Model>)>>,
    models: Query<&Model>,
    window: Option<Single<&Window, With<PrimaryWindow>>>,
    mut targets: ResMut<Assets<Image>>,
    mut layers: ResMut<Assets<Layer>>,
) {
    if !pack.is_changed() {
        return;
    }
    let key = pack.held.map(|held| held.key);
    if models.iter().map(|model| model.key).next() == key {
        return;
    }
    for view in &views {
        commands.entity(view).despawn();
    }
    let (Some(held), Some(tables), Some(window)) = (pack.held, tables, window) else {
        return;
    };
    let Some((model, texture, pose)) = tables.look(held.dropped.item) else {
        return;
    };
    let pixels = UVec2::new(window.physical_width(), window.physical_height()).max(UVec2::ONE);
    let scale = window.height() / crate::bar::SCREEN.y;
    let size = Vec2::new(window.width(), window.height()) / scale.max(0.01);
    let target = targets.add(Image::new_target_texture(
        pixels.x,
        pixels.y,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands.spawn((
        View,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        MaterialNode(layers.add(Layer {
            figures: target.clone(),
        })),
        GlobalZIndex(OVER_ALL),
        Pickable::IGNORE,
        DespawnOnExit(AppState::InGame),
    ));
    commands.spawn((
        View,
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
        RenderLayers::layer(HELD),
        Apart,
        DespawnOnExit(AppState::InGame),
    ));
    commands.spawn((
        View,
        DirectionalLight {
            illuminance: backpack::LIGHT,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-0.5, 1.0, 1.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(HELD),
        DespawnOnExit(AppState::InGame),
    ));
    let mut entity = commands.spawn((
        Model {
            key: held.key,
            size: held.size,
            pose,
        },
        Matte,
        crate::models::scene(&assets, model),
        Transform::default(),
        Visibility::Hidden,
        RenderLayers::layer(HELD),
        DespawnOnExit(AppState::InGame),
    ));
    if let Some(texture) = texture {
        entity.insert(Retexture::load(&assets, texture));
    }
}

/// Keeps the held item's model at the cursor: its rectangle centred on it, on the window's pixels
/// of the original's screen; hidden while the cursor is off the window.
fn follow(
    pointer: Res<Pointer>,
    ui_scale: Res<UiScale>,
    mut models: Query<(&Model, &mut Transform, &mut Visibility)>,
) {
    for (model, mut transform, mut visibility) in &mut models {
        let Some(at) = pointer.0 else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        let size = Vec2::new(f32::from(model.size.0), f32::from(model.size.1)) * backpack::CELL;
        let corner = at / ui_scale.0.max(0.01) - size / 2.0;
        let placed = Transform::from_xyz(corner.x, -corner.y, 0.0)
            * backpack::placement((0, 0), model.size, model.pose);
        transform.set_if_neq(placed);
        visibility.set_if_neq(Visibility::Inherited);
    }
}

/// Hides the system's cursor while an item is held: the item is the cursor.
fn cursor(pack: Res<Pack>, mut options: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    let visible = pack.held.is_none();
    for mut options in &mut options {
        if options.visible != visible {
            options.visible = visible;
        }
    }
}

/// A left click on the world with an item held drops it beside the hero (`cEngine::dropItem`,
/// `0x611620`): on the first cell about the hero's, from the one toward the click, that may be
/// walked on, else the hero's own; the click does nothing else.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn drop_held(
    mut commands: Commands,
    mut buttons: ResMut<ButtonInput<MouseButton>>,
    mut pack: ResMut<Pack>,
    pointer: Res<Pointer>,
    camera: Single<(&Camera, &GlobalTransform), crate::iso::Eye<With<Camera>>>,
    (heights, buildings): (Res<Heights>, Res<Buildings>),
    heroes: Query<(&Transform, &Body)>,
    (assets, tables, mut dice): (Res<AssetServer>, Option<Res<Tables>>, ResMut<Dice>),
) {
    if pack.held.is_none() || !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let (Some(tables), Some((transform, body))) = (tables, heroes.iter().next()) else {
        return;
    };
    buttons.reset(MouseButton::Left);
    let here = transform.translation;
    let level = body.level();
    let hero_cell = paths::cell_of(here.x, here.z);
    let (camera, view) = *camera;
    let toward = pointer
        .0
        .and_then(|cursor| camera.viewport_to_world(view, cursor).ok())
        .map(
            |ray| match pick::pick(ray, &heights, &buildings, (here.y, level)) {
                Some(picked) => picked.point,
                None => {
                    let along = (here.y - ray.origin.y) / ray.direction.y;
                    ray.origin + ray.direction.as_vec3() * along
                }
            },
        )
        .map_or(hero_cell, |point| paths::cell_of(point.x, point.z));
    let cell = drop_cell(hero_cell, toward, |cell| heights.walkable_at(cell, level));
    let Some(held) = pack.held.take() else {
        return;
    };
    info!(
        "held: {} dropped at {cell:?} beside the hero at {hero_cell:?}, toward {toward:?}",
        held.dropped.item
    );
    drops::drop_at(
        &mut commands,
        (&assets, &tables, &mut dice),
        (cell, level),
        held.dropped,
    );
}

/// The cursor's item.
pub struct HeldPlugin;

impl Plugin for HeldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                click.in_set(Click).before(show),
                (show, follow, cursor)
                    .chain()
                    .before(backpack::layer)
                    .after(crate::inventory::Opened),
                drop_held
                    .in_set(Control)
                    .in_set(Unpaused)
                    .before(attack::aim)
                    .before(crate::making::aim)
                    .before(hero::steer),
            )
                .run_if(in_state(AppState::InGame)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_item_put_sounds_by_its_category() {
        assert_eq!(put_sound(9).0, 7019);
        assert_eq!(put_sound(13).0, 7115);
        assert_eq!(put_sound(24).0, 7116);
        let (metal, (base, _)) = put_sound(5);
        assert_eq!((metal, base), (7005, 0.45));
    }
}
