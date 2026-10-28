//! The equipment window's slots (#1288, #1289; docs/re/inventory.md, §4): each class's frames of
//! its slots at their places in the window (`opensacred_rules::equipment::frames`), drawn white at
//! the alpha `0xAF`, and their glow while the cursor holds an item: green where the slot takes it
//! and the hero may use it, red otherwise (`0xAF00FF00`, `0xAFFF0000`; seen in the original with a
//! potion held, every slot red).
//!
//! The clicks (`0x6B6690`): the left button on a worn item takes it off onto the empty cursor;
//! with an item held, a slot that takes it puts it on with its category's sound (`(9, 5)`), the old
//! item onto the cursor, where the hero may use it, else the hero refuses it by its line
//! (`NOTMYITEM`, `NOTMYITEMLVL`); the right button on a worn item puts it into the grid at its
//! first fit, and on an item of the grid puts it on in the first empty slot that takes it, else
//! the first, the old item into the cell it left. The worn items are their 3D models centred in
//! their frames, drawn by a camera of their own as the grid's are (`backpack`).
//!
//! The hero's fight takes the worn items' values (#1290, `0x5796A0`): its attack, defence,
//! damage, resistances and regeneration made anew with them, its effects, at its level, as its
//! level-up makes them.
//!
//! The hero wears them (#1307, `GrannyWearSequence`): their models on its skeleton, the wings on
//! its back, each taking off its template's item in its slot, a class's default, which comes back
//! as the slot empties (`0x556A30`); the window does not show the defaults. The items in its hands
//! are the weapon sets' (#1271), so that its clips do not change by the window.
//!
//! Not followed: the amulets and the rings on its neck and fingers.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{RenderTarget, ScalingMode};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::ui_render::prelude::MaterialNode;
use opensacred_rules::equipment::{Glow, Refusal, frames, glow, refusal, slot_for, takes};
use opensacred_rules::fight::{combat_with, regeneration_with};

use crate::backpack::{self, Matte, cell_under, on_grid};
use crate::bar::{self, UiImages};
use crate::choice::Layer;
use crate::drops::Tables;
use crate::effects::Effects;
use crate::fight::{self, Base, Combat, Experience, Holding, Regenerates, Rules};
use crate::game::AppState;
use crate::hero::Body;
use crate::hero::Hero;
use crate::hud::Pointer;
use crate::inventory::{EQUIPMENT, EquipmentWindow, Shown};
use crate::iso::Apart;
use crate::npcs::Retexture;
use crate::pickup::Pack;
use crate::sfx;
use crate::voices::NotMine;

/// The frames' alpha (`0xAF`).
const ALPHA: f32 = 175.0 / 255.0;
/// The render layer of the worn items' models.
const WORN: usize = 12;

/// A slot's frame, of its slot, and its rectangle in the window.
#[derive(Component)]
struct SlotFrame(u8, Rect);

/// The worn items' camera, light and models.
#[derive(Component)]
struct Scene;

/// A worn item's model, by its key and its slot.
#[derive(Component)]
struct Model(u32, u8);

/// Gives the equipment window its class's frames as it is made, and the picture that its worn
/// items' camera draws, over them.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn show(
    mut commands: Commands,
    windows: Query<Entity, Added<EquipmentWindow>>,
    hero: Option<Res<Hero>>,
    (images, assets): (Res<UiImages>, Res<AssetServer>),
    mut targets: ResMut<Assets<Image>>,
    mut layers: ResMut<Assets<Layer>>,
    window: Option<Single<&Window>>,
    olds: Query<Entity, With<Scene>>,
) {
    let Some(hero) = hero else {
        return;
    };
    for equipment in &windows {
        for old in &olds {
            commands.entity(old).despawn();
        }
        for frame in frames(hero.creature) {
            let at = Vec2::new(f32::from(frame.at.0), f32::from(frame.at.1));
            let (mut image, node) = bar::piece(&images, &assets, frame.image, at, Vec2::ZERO);
            let size = match (node.width, node.height) {
                (Val::Px(width), Val::Px(height)) => Vec2::new(width, height),
                _ => Vec2::splat(64.0),
            };
            image.color = Color::srgba(1.0, 1.0, 1.0, ALPHA);
            let child = commands
                .spawn((
                    image,
                    node,
                    SlotFrame(frame.slot, Rect::from_corners(at, at + size)),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(equipment).add_child(child);
        }
        // The worn items' picture, of the window's pixels, over the frames.
        let size = EQUIPMENT.size();
        let scale = window
            .as_ref()
            .map_or(1.0, |window| window.height() / bar::SCREEN.y);
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
        commands.entity(equipment).add_child(picture);
        commands.spawn((
            Scene,
            Camera3d::default(),
            Camera {
                order: -4,
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
            RenderLayers::layer(WORN),
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
            RenderLayers::layer(WORN),
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// Takes the worn items' camera and models away as the window goes.
fn hide(
    mut commands: Commands,
    windows: Query<(), With<EquipmentWindow>>,
    scenes: Query<Entity, With<Scene>>,
) {
    if !windows.is_empty() {
        return;
    }
    for scene in &scenes {
        commands.entity(scene).despawn();
    }
}

/// Lays the worn items' models out in their frames as the pack holds them: each centred in its
/// frame, turned by its type's pose.
fn lay_out(
    mut commands: Commands,
    pack: Res<Pack>,
    (tables, assets): (Option<Res<Tables>>, Res<AssetServer>),
    frames: Query<&SlotFrame>,
    models: Query<(Entity, &Model)>,
    camera: Query<(), (With<Scene>, With<Camera>)>,
) {
    let (Some(tables), false) = (tables, camera.is_empty()) else {
        return;
    };
    let worn: Vec<(u32, u8, u32)> = pack
        .worn
        .iter()
        .enumerate()
        .filter_map(|(slot, worn)| {
            let (key, dropped) = worn.as_ref()?;
            Some((*key, u8::try_from(slot).ok()?, dropped.item))
        })
        .collect();
    for (entity, model) in &models {
        if !worn
            .iter()
            .any(|&(key, slot, _)| key == model.0 && slot == model.1)
        {
            commands.entity(entity).despawn();
        }
    }
    for (key, slot, item) in worn {
        if models
            .iter()
            .any(|(_, model)| model.0 == key && model.1 == slot)
        {
            continue;
        }
        let Some(frame) = frames.iter().find(|frame| frame.0 == slot) else {
            continue;
        };
        let Some((model, texture, pose)) = tables.look(item) else {
            continue;
        };
        let centre = frame.1.center();
        // The pose about the model's origin, its rectangle's centre at the frame's.
        let placed = Transform::from_xyz(
            centre.x - backpack::CELL / 2.0,
            -(centre.y - backpack::CELL / 2.0),
            0.0,
        ) * backpack::placement((0, 0), (1, 1), pose);
        let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(model.to_owned()));
        let mut entity = commands.spawn((
            Scene,
            Model(key, slot),
            Matte,
            WorldAssetRoot(scene),
            placed,
            RenderLayers::layer(WORN),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(texture) = texture {
            entity.insert(Retexture::load(&assets, texture));
        }
    }
}

/// Tints each frame by its glow for the item the cursor holds, the hero's type and level.
fn tint(
    pack: Res<Pack>,
    hero: Option<Res<Hero>>,
    (experience, rules, tables): (Res<Experience>, Res<Rules>, Option<Res<Tables>>),
    mut frames: Query<(&SlotFrame, &mut ImageNode)>,
) {
    let Some(hero) = hero else {
        return;
    };
    let asks = pack
        .held
        .zip(tables.as_deref())
        .map(|(held, tables)| tables.asks(&held.dropped, &rules));
    for (frame, mut image) in &mut frames {
        let wanted = match glow(frame.0, asks.as_ref(), (hero.creature, experience.0.level)) {
            Glow::White => Color::srgba(1.0, 1.0, 1.0, ALPHA),
            Glow::Green => Color::srgba(0.0, 1.0, 0.0, ALPHA),
            Glow::Red => Color::srgba(1.0, 0.0, 0.0, ALPHA),
        };
        if image.color != wanted {
            image.color = wanted;
        }
    }
}

/// The clicks on the equipment window and the right button on the grid (`0x6B6690`, the UI event
/// `0x2C`).
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn click(
    shown: Res<Shown>,
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale): (Res<Pointer>, Res<UiScale>),
    windows: Query<&Window>,
    mut pack: ResMut<Pack>,
    (hero, experience, rules, tables): (
        Option<Res<Hero>>,
        Res<Experience>,
        Res<Rules>,
        Option<Res<Tables>>,
    ),
    frames: Query<&SlotFrame>,
    (mut sounds, mut refusals): (MessageWriter<sfx::Play>, MessageWriter<NotMine>),
) {
    let (left, right) = (
        buttons.just_pressed(MouseButton::Left),
        buttons.just_pressed(MouseButton::Right),
    );
    let (true, true, Some(hero), Some(tables)) = (shown.0, left || right, hero, tables) else {
        return;
    };
    let level = experience.0.level;
    let size = |item: u32| tables.size(item);
    let put = |sounds: &mut MessageWriter<sfx::Play>, item: u32| {
        let (id, rate) = crate::held::put_sound(tables.category(item));
        sounds.write(sfx::Play {
            sound: sfx::Of::Id(id),
            at: None,
            rate,
        });
    };
    // The right button on an item of the grid puts it on, the cursor empty.
    if right
        && pack.held.is_none()
        && let Some(cell) = on_grid(&pointer, &ui_scale, &windows).and_then(cell_under)
    {
        let Some(&placed) = pack.grid.at(cell) else {
            return;
        };
        let Some(dropped) = pack
            .items
            .iter()
            .find(|(key, _)| *key == placed.key)
            .map(|(_, dropped)| *dropped)
        else {
            return;
        };
        let asks = tables.asks(&dropped, &rules);
        let Some(slot) = slot_for(asks.category, |slot| {
            pack.worn
                .get(usize::from(slot))
                .is_some_and(Option::is_none)
                && frames.iter().any(|frame| frame.0 == slot)
        }) else {
            return;
        };
        if let Some(why) = refusal(hero.creature, level, &asks) {
            debug!("equipment: {} refused: {why:?}", dropped.item);
            refusals.write(NotMine {
                level: why == Refusal::Level,
            });
            return;
        }
        let old_size = pack
            .worn
            .get(usize::from(slot))
            .copied()
            .flatten()
            .map_or((1, 1), |(_, old)| size(old.item));
        if pack.wear(placed.key, slot, old_size) {
            debug!("equipment: {} put on in {slot}", dropped.item);
            put(&mut sounds, dropped.item);
        }
        return;
    }
    // A click on a slot of the window.
    let Some(at) = crate::inventory::screen_point(&pointer, &ui_scale, &windows) else {
        return;
    };
    let local = at - EQUIPMENT.min;
    let Some(slot) = frames
        .iter()
        .find(|frame| frame.1.contains(local))
        .map(|frame| frame.0)
    else {
        return;
    };
    let worn = pack.worn.get(usize::from(slot)).copied().flatten();
    if right {
        if let Some((_, dropped)) = worn
            && pack.unwear(slot, size(dropped.item))
        {
            debug!("equipment: {} taken off into the grid", dropped.item);
        }
        return;
    }
    let Some(held) = pack.held else {
        if let Some((_, dropped)) = worn
            && pack.take_off(slot, size(dropped.item))
        {
            debug!("equipment: {} taken off onto the cursor", dropped.item);
        }
        return;
    };
    let asks = tables.asks(&held.dropped, &rules);
    if let Some(why) = refusal(hero.creature, level, &asks) {
        debug!("equipment: {} refused: {why:?}", held.dropped.item);
        refusals.write(NotMine {
            level: why == Refusal::Level,
        });
        return;
    }
    if !takes(slot, asks.category) {
        return;
    }
    let old_size = worn.map_or((1, 1), |(_, old)| size(old.item));
    if pack.put_on(slot, old_size) {
        debug!("equipment: {} put on in {slot}", held.dropped.item);
        put(&mut sounds, held.dropped.item);
    }
}

/// The values of the items that the hero wears in the window (#1290), the last of its
/// `Holding`'s items, after those of its arms: what a change of its arms keeps (`arsenal`).
#[derive(Component, Clone, Debug, Default)]
pub struct Worn(pub Vec<fight::ItemValues>);

/// Makes the hero's fight anew as the items it wears in the window change (`0x5796A0`): its
/// arms' items and the worn ones' values, its effects, at its level.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn refit(
    mut commands: Commands,
    pack: Res<Pack>,
    (experience, rules, tables): (Res<Experience>, Res<Rules>, Option<Res<Tables>>),
    mut hero: Option<ResMut<Hero>>,
    mut heroes: Query<
        (
            Entity,
            &Base,
            &mut Holding,
            &mut Combat,
            &mut Regenerates,
            Option<&Effects>,
            Option<&Worn>,
        ),
        With<Body>,
    >,
    mut seen: Local<Vec<u32>>,
) {
    let Some(tables) = tables else {
        return;
    };
    let worn: Vec<u32> = pack.worn.iter().flatten().map(|&(key, _)| key).collect();
    if *seen == worn {
        return;
    }
    for (entity, base, mut holding, mut combat, mut regenerates, effects, old) in &mut heroes {
        let values: Vec<fight::ItemValues> = pack
            .worn
            .iter()
            .flatten()
            .map(|(_, d)| tables.values(d))
            .collect();
        let mut holds = holding.0.clone();
        let arms = holds
            .items
            .len()
            .saturating_sub(old.map_or(0, |old| old.0.len()));
        holds.items.truncate(arms);
        holds.items.extend(values.iter().cloned());
        commands.entity(entity).insert(Worn(values));
        let level = experience.level;
        let skills = fight::skills(base, level, true);
        let list = effects.map(Effects::list).unwrap_or_default();
        *combat = Combat(combat_with(
            base,
            level,
            holds.clone(),
            &skills,
            &list,
            &rules,
        ));
        regenerates.value = regeneration_with(base, level, &skills, &list, &rules);
        holding.0 = holds;
        if let Some(hero) = hero.as_mut() {
            hero.pace = combat.pace_factor();
        }
        info!(
            "equipment: the hero wears {} items: attack {:.2}, defence {:.2}, resistances {:?}",
            worn.len(),
            combat.attack,
            combat.defence,
            combat.resistance
        );
    }
    *seen = worn;
}

/// Whether the hero shows the item worn in the slot `slot` of its equipment (#1307): those of
/// the slots 0 to 5 and 14 to 16 on its skeleton and the wings of 17 on its back; the amulets and
/// the rings (6 to 11), which the original carries on its neck and fingers, not.
fn shows(slot: u8) -> bool {
    matches!(slot, 0..=5 | 14..=17)
}

/// Dresses the hero in the items it wears in the window as they change (#1307): each one's model
/// and texture on it ([`shows`]), its template's items in their slots taken off and given back
/// as the slots empty, as the classes' defaults are (`0x556A30`).
fn dress(
    pack: Res<Pack>,
    tables: Option<Res<Tables>>,
    hero: Option<ResMut<Hero>>,
    assets: Res<AssetServer>,
    mut seen: Local<Vec<(u8, u32)>>,
) {
    let (Some(tables), Some(mut hero)) = (tables, hero) else {
        return;
    };
    let worn: Vec<(u8, u32)> = pack
        .worn
        .iter()
        .zip(0u8..)
        .filter(|&(_, slot)| shows(slot))
        .filter_map(|(worn, slot)| Some((slot, worn.as_ref()?.1.item)))
        .collect();
    if *seen == worn {
        return;
    }
    let wears = worn
        .iter()
        .filter_map(|&(slot, item)| {
            let (model, texture, _) = tables.look(item)?;
            Some(crate::npcs::Held::worn(
                slot,
                model.to_owned(),
                texture.map(str::to_owned),
            ))
        })
        .collect();
    debug!("equipment: the hero is dressed in {worn:?}");
    hero.dress(wears, &assets);
    *seen = worn;
}

/// The equipment window's slots.
pub struct EquipmentPlugin;

impl Plugin for EquipmentPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                (show, hide, click, lay_out, tint)
                    .chain()
                    .after(crate::inventory::Opened)
                    .before(backpack::layer),
                (refit, dress).after(click),
            )
                .run_if(in_state(AppState::InGame)),
        );
    }
}
