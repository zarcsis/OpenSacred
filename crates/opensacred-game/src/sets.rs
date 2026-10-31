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
//!
//! A click on a shown slot, the cursor empty, or the keys 1 to 5 choose its set (`0x6E1690`,
//! #1348): the slot takes the selected orb and ring, and the set's items go into the hero's hands
//! (`sdk_EQUIP` of the slots 12 and 13), its arms made anew with them (`arsenal::Maker`): their
//! models, the clips of their class, its values and its weapons' sounds; a change of the class
//! stops the hero (`0x54E6D0`). A set of nothing leaves its hands empty.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{RenderTarget, ScalingMode};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::ui_render::prelude::MaterialNode;
use opensacred_data::heroes::Worn;
use opensacred_rules::drops::{Dropped, Kind};
use opensacred_rules::sets as rule;

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

/// The hero's weapon sets: each set's two halves, the left hand's and the right's, and the set
/// selected (`0x5D9BA0(0)`).
#[derive(Resource, Default, Clone, Debug)]
pub struct WeaponSets {
    pub halves: [[Option<Dropped>; HALVES]; SETS],
    pub selected: usize,
}

/// A slot's side on the original's screen.
const SLOT: f32 = 64.0;

/// A set's half's model: its item, at its place in the picture.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
struct Model {
    item: u32,
    at: Vec3,
}

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
            named: None,
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
            ZIndex(bar::RING_Z + 1),
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

/// The models that the shown sets' slots draw, each of its item at its place.
fn models_of(sets: &WeaponSets, shown: usize, tables: &Tables) -> Vec<Model> {
    let mut models = Vec::new();
    for (set, halves) in sets.halves.iter().enumerate().take(shown) {
        let both_weapons = halves
            .iter()
            .all(|half| half.is_some_and(|item| tables.category(item.item) == WEAPON));
        let corner = bar::weapon_slot(set, shown);
        for (half, item) in halves.iter().enumerate() {
            let Some(item) = item else {
                continue;
            };
            models.push(Model {
                item: item.item,
                at: origin(corner, tables.in_slot(item.item), half, both_weapons),
            });
        }
    }
    models
}

/// The keys of the models of `had` that `wanted` no longer holds, and the models of `wanted` not
/// among `had`, which alone are made anew.
fn changed<K: Copy>(had: &[(K, Model)], mut wanted: Vec<Model>) -> (Vec<K>, Vec<Model>) {
    let mut gone = Vec::new();
    for &(key, model) in had {
        match wanted.iter().position(|&kept| kept == model) {
            Some(kept) => {
                wanted.swap_remove(kept);
            }
            None => gone.push(key),
        }
    }
    (gone, wanted)
}

/// Lays the shown sets' models out in their slots as the sets or the slots shown change: the
/// models of the items gone or moved despawned, those of the items come or moved spawned, the rest
/// left drawn, as a new model's scene takes frames to load (a set's choice moves none).
fn lay_out(
    mut commands: Commands,
    sets: Option<Res<WeaponSets>>,
    shown: Option<Res<bar::Shown>>,
    (tables, assets): (Option<Res<Tables>>, Res<AssetServer>),
    models: Query<(Entity, &Model)>,
) {
    let (Some(sets), Some(shown), Some(tables)) = (sets, shown, tables) else {
        return;
    };
    if !(sets.is_changed() || shown.is_changed()) {
        return;
    }
    let had: Vec<(Entity, Model)> = models
        .iter()
        .map(|(entity, &model)| (entity, model))
        .collect();
    let (gone, new) = changed(&had, models_of(&sets, shown.0, &tables));
    for entity in gone {
        commands.entity(entity).despawn();
    }
    for new in new {
        let Some((model, texture, pose)) = tables.look(new.item) else {
            continue;
        };
        let mut entity = commands.spawn((
            new,
            Matte,
            crate::models::scene(&assets, model),
            backpack::posed(new.at, pose),
            RenderLayers::layer(MODELS),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(texture) = texture {
            entity.insert(Retexture::load(&assets, texture));
        }
    }
}

/// The set of the shown slot under the screen's point `at`, of `count` shown.
fn slot_at(at: Vec2, count: usize) -> Option<usize> {
    (0..count.min(SETS)).find(|&set| {
        let corner = bar::weapon_slot(set, count);
        Rect::from_corners(corner, corner + Vec2::splat(SLOT)).contains(at)
    })
}

/// Chooses a weapon set by a left click on its slot, or by its key while the console is closed
/// (`0x6E1690`): one of the slots shown alone. While the inventory shows, the click first puts
/// the cursor's item into the set or takes one out of it ([`edit`]), the sound `(9, 3)` where
/// one moved. The bar holds the player's control of the world back under the pointer
/// (`frame::Control`), so the click is taken here.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn choose(
    keys: crate::controls::Keys,
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, console, inventory): (
        Res<crate::hud::Pointer>,
        Res<UiScale>,
        Option<Res<crate::console::Console>>,
        Option<Res<crate::inventory::Shown>>,
    ),
    windows: Query<&Window>,
    (shown, pack, sets, tables): (
        Option<Res<bar::Shown>>,
        Option<ResMut<crate::pickup::Pack>>,
        Option<ResMut<WeaponSets>>,
        Option<Res<Tables>>,
    ),
    (hero, experience, rules): (
        Option<Res<crate::hero::Hero>>,
        Res<crate::fight::Experience>,
        Res<crate::fight::Rules>,
    ),
    (mut sounds, mut refusals): (
        MessageWriter<crate::sfx::Play>,
        MessageWriter<crate::voices::NotMine>,
    ),
) {
    let (Some(shown), Some(mut sets)) = (shown, sets) else {
        return;
    };
    let clicked = buttons
        .just_pressed(MouseButton::Left)
        .then(|| crate::inventory::screen_point(&pointer, &ui_scale, &windows))
        .flatten()
        .and_then(|at| Some((slot_at(at, shown.0)?, at)));
    let editing = inventory.is_some_and(|inventory| inventory.0);
    if let (Some((set, at)), true, Some(mut pack), Some(tables), Some(hero)) =
        (clicked, editing, pack, tables, hero)
        && edit(
            set,
            at,
            shown.0,
            &mut sets,
            &mut pack,
            &tables,
            (hero.creature, experience.0.level, &rules),
            &mut refusals,
        )
    {
        sounds.write(put_sound());
    }
    // The keys that choose the weapon sets 1 to 5 (`0x6E1BC0`).
    let keyed = crate::controls::WEAPON_SETS
        .iter()
        .position(|&action| keys.just_pressed(action))
        .filter(|_| console.is_none_or(|console| !console.is_open()));
    if let Some(set) = clicked
        .map(|(set, _)| set)
        .or(keyed)
        .filter(|&set| set < shown.0)
        && set != sets.selected
    {
        sets.selected = set;
        info!("sets: the weapon set {} chosen", set + 1);
    }
}

/// The categories that a weapon set takes (`items.pak +0x2E`, `0x6E09C0`): the weapons and the
/// shields.
const SHIELD: u8 = 13;
/// The key of a piece put into a set, beside its halves' 0 and 1.
const PUT: u32 = 2;

/// An item as a piece of a set of the key `key`: a shield, or a weapon of its kind's hands and
/// hold; none for any other.
fn piece(tables: &Tables, key: u32, item: u32) -> Option<rule::Piece> {
    match tables.category(item) {
        WEAPON => Some(rule::Piece::weapon(key, tables.weapon_kind(item), item)),
        SHIELD => Some(rule::Piece::shield(key)),
        _ => None,
    }
}

/// A set's halves as the rule's pieces, keyed by their halves.
fn pieces(tables: &Tables, halves: &[Option<Dropped>; HALVES]) -> rule::Set {
    [0, 1].map(|half| {
        halves[half].and_then(|item| piece(tables, u32::try_from(half).unwrap_or(0), item.item))
    })
}

/// The item of the piece `piece`: a half's of `halves` by its key, else `put`.
fn item_of(
    piece: rule::Piece,
    halves: &[Option<Dropped>; HALVES],
    put: Option<Dropped>,
) -> Option<Dropped> {
    match piece.key {
        PUT => put,
        key => halves[usize::try_from(key).unwrap_or(0).min(HALVES - 1)],
    }
}

/// What a set made of its rule's pieces holds.
fn halves_of(
    set: rule::Set,
    halves: &[Option<Dropped>; HALVES],
    put: Option<Dropped>,
) -> [Option<Dropped>; HALVES] {
    set.map(|piece| piece.and_then(|piece| item_of(piece, halves, put)))
}

/// The halves of a slot that a click on its point `at` tries, in order (`0x6E14C0`): the right
/// hand's first on the slot's left half, as its model lies there, else the left hand's.
fn halves_tried(corner: Vec2, at: Vec2) -> [usize; HALVES] {
    if at.x < corner.x + SLOT / 2.0 {
        [1, 0]
    } else {
        [0, 1]
    }
}

/// How a put of an item into a set was refused, by the hero's line (`0x6E14C0`, the message 5):
/// `NOTMYITEMLVL` where only the level or the attributes refuse it, else `NOTMYITEM`; or none
/// where the hero may use it.
fn refused(
    tables: &Tables,
    dropped: &Dropped,
    hero: u32,
    level: u32,
    rules: &crate::fight::Rules,
) -> Option<bool> {
    let asks = tables.asks(dropped, rules);
    opensacred_rules::equipment::refusal(hero, level, &asks)
        .map(|why| why == opensacred_rules::equipment::Refusal::Level)
}

/// A left click on a set's slot while the inventory shows (its edit mode, `0x6E14C0`, seen): the
/// cursor empty, the item of the half under the click, else of the other, onto the cursor, the
/// left hand's weapon moving into the right hand where the right's goes (`0x6E0E30`); an item
/// held, put into the half under the click, else the other, by the rule (`0x6E09C0`), the item it
/// replaced onto the cursor, or refused by the hero's line. Either way the set is chosen with the
/// sound `(9, 3)`. Whether anything moved.
#[expect(
    clippy::too_many_arguments,
    reason = "the click's set and place, the sets, the pack and what the rule and the use's test ask"
)]
fn edit(
    set: usize,
    at: Vec2,
    count: usize,
    sets: &mut WeaponSets,
    pack: &mut crate::pickup::Pack,
    tables: &Tables,
    (hero, level, rules): (u32, u32, &crate::fight::Rules),
    refusals: &mut MessageWriter<crate::voices::NotMine>,
) -> bool {
    let tried = halves_tried(bar::weapon_slot(set, count), at);
    let halves = sets.halves[set];
    let Some(held) = pack.held else {
        let Some(half) = tried.into_iter().find(|&half| halves[half].is_some()) else {
            return false;
        };
        let (left, taken) = rule::take(pieces(tables, &halves), half);
        let Some(taken) = taken.and_then(|piece| item_of(piece, &halves, None)) else {
            return false;
        };
        sets.halves[set] = halves_of(left, &halves, None);
        pack.hold(taken, tables.size(taken.item));
        info!("sets: {} taken out of the set {}", taken.item, set + 1);
        return true;
    };
    if let Some(level_only) = refused(tables, &held.dropped, hero, level, rules) {
        refusals.write(crate::voices::NotMine { level: level_only });
        return false;
    }
    let made = piece(tables, PUT, held.dropped.item)
        .and_then(|piece| rule::drop(pieces(tables, &halves), tried[0], piece, false));
    let Some((_, (made, replaced))) = made else {
        refusals.write(crate::voices::NotMine { level: false });
        return false;
    };
    sets.halves[set] = halves_of(made, &halves, Some(held.dropped));
    pack.held = None;
    if let Some(replaced) = replaced.and_then(|piece| item_of(piece, &halves, None)) {
        pack.hold(replaced, tables.size(replaced.item));
    }
    info!("sets: {} put into the set {}", held.dropped.item, set + 1);
    true
}

/// The right button on a weapon or a shield of the grid while the inventory shows, the cursor
/// empty (`0x6C0150`, the taskbar's message 5, `0x6E0E30`): into the selected set, its left half
/// tried first, the item it replaced back into the grid at its first fit, with the sound `(9, 3)`;
/// else the hero's line, `CANTCARRYMORE` where it may use the item (`(8, 3)`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn send(
    (shown, page): (Res<crate::inventory::Shown>, Res<crate::inventory::Page>),
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale): (Res<crate::hud::Pointer>, Res<UiScale>),
    windows: Query<&Window>,
    (pack, sets, tables): (
        Option<ResMut<crate::pickup::Pack>>,
        Option<ResMut<WeaponSets>>,
        Option<Res<Tables>>,
    ),
    (hero, experience, rules): (
        Option<Res<crate::hero::Hero>>,
        Res<crate::fight::Experience>,
        Res<crate::fight::Rules>,
    ),
    (mut sounds, mut refusals, mut remarks): (
        MessageWriter<crate::sfx::Play>,
        MessageWriter<crate::voices::NotMine>,
        MessageWriter<crate::remarks::Remark>,
    ),
) {
    let (true, true, Some(mut pack), Some(mut sets), Some(tables), Some(hero)) = (
        shown.0,
        buttons.just_pressed(MouseButton::Right),
        pack,
        sets,
        tables,
        hero,
    ) else {
        return;
    };
    let Some(cell) =
        backpack::on_grid(&pointer, &ui_scale, &windows, &page).and_then(backpack::cell_under)
    else {
        return;
    };
    let Some(placed) = pack.grid.at(cell).copied() else {
        return;
    };
    let Some(dropped) = pack
        .items
        .iter()
        .find(|(key, _)| *key == placed.key)
        .map(|&(_, dropped)| dropped)
    else {
        return;
    };
    if pack.held.is_some() || !matches!(tables.category(dropped.item), WEAPON | SHIELD) {
        return;
    }
    if let Some(level_only) = refused(&tables, &dropped, hero.creature, experience.0.level, &rules)
    {
        refusals.write(crate::voices::NotMine { level: level_only });
        return;
    }
    let set = sets.selected;
    let halves = sets.halves[set];
    let made = piece(&tables, PUT, dropped.item)
        .and_then(|piece| rule::drop(pieces(&tables, &halves), 0, piece, false));
    let Some((_, (made, replaced))) = made else {
        remarks.write(crate::remarks::Remark::CantCarry);
        return;
    };
    let replaced = replaced.and_then(|piece| item_of(piece, &halves, None));
    pack.use_up(placed.key);
    if let Some(replaced) = replaced
        && !pack.take(replaced, tables.size(replaced.item))
    {
        // No room for the item it replaced: nothing moves.
        pack.put(dropped, placed.at, placed.size);
        return;
    }
    sets.halves[set] = halves_of(made, &halves, Some(dropped));
    sounds.write(put_sound());
    info!("sets: {} sent into the set {}", dropped.item, set + 1);
}

/// The halves' glow while the cursor holds an item that a half takes (`0x6E29E0`, docs/re/
/// interface.md, "The bar"): the selected set's left and right halves the images 531 and 532,
/// another set's 549 and 550; the slot's left half the right hand's.
const GLOWS: [[u32; HALVES]; 2] = [[531, 532], [549, 550]];

/// A half's glow over the bar.
#[derive(Component)]
struct Glow;

/// Lays the halves' glow over the shown slots while the inventory shows and the cursor holds an
/// item that a half takes by the rule (`0x6E09C0`'s test), the hero able to use it: the slot's
/// left half where the right hand takes it, its right half where the left hand does.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn glow(
    mut commands: Commands,
    inventory: Option<Res<crate::inventory::Shown>>,
    (pack, sets, shown, tables): (
        Option<Res<crate::pickup::Pack>>,
        Option<Res<WeaponSets>>,
        Option<Res<bar::Shown>>,
        Option<Res<Tables>>,
    ),
    (hero, experience, rules): (
        Option<Res<crate::hero::Hero>>,
        Res<crate::fight::Experience>,
        Res<crate::fight::Rules>,
    ),
    (images, assets): (Res<bar::UiImages>, Res<AssetServer>),
    bars: Query<Entity, With<BarRoot>>,
    glows: Query<Entity, With<Glow>>,
    mut last: Local<Option<(Option<u32>, usize, usize, bool)>>,
) {
    let (Some(pack), Some(sets), Some(shown), Some(tables), Some(hero), Ok(bar)) =
        (pack, sets, shown, tables, hero, bars.single())
    else {
        return;
    };
    let editing = inventory.is_some_and(|inventory| inventory.0);
    let held = pack.held.filter(|_| editing);
    let now = (
        held.map(|held| held.key),
        sets.selected,
        shown.0,
        sets.is_changed(),
    );
    if *last == Some(now) && !sets.is_changed() {
        return;
    }
    *last = Some((now.0, now.1, now.2, false));
    for glow in &glows {
        commands.entity(glow).despawn();
    }
    let Some(held) = held else {
        return;
    };
    let level = experience.0.level;
    if refused(&tables, &held.dropped, hero.creature, level, &rules).is_some() {
        return;
    }
    let Some(piece) = piece(&tables, PUT, held.dropped.item) else {
        return;
    };
    for (set, halves) in sets.halves.iter().enumerate().take(shown.0) {
        let corner = bar::weapon_slot(set, shown.0);
        let images_of = GLOWS[usize::from(set != sets.selected)];
        for half in 0..HALVES {
            if rule::put(pieces(&tables, halves), half, piece, false).is_none() {
                continue;
            }
            // The right hand's on the slot's left half.
            let (image, left) = if half == 1 {
                (images_of[0], 0.0)
            } else {
                (images_of[1], SLOT / 2.0)
            };
            let at = corner + Vec2::new(left, 0.0);
            let (node_image, node) = bar::piece(&images, &assets, image, at, bar::BAR.min);
            let child = commands
                .spawn((
                    Glow,
                    node_image,
                    node,
                    ZIndex(bar::RING_Z - 1),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(bar).add_child(child);
        }
    }
}

/// The sound `(9, 3)` of an item put into a set or taken out, in the middle: `UI_INV_PUTMETAL01`
/// at its rate of a metal item put down (`held::put_sound`).
fn put_sound() -> crate::sfx::Play {
    let (id, rate) = crate::held::put_sound(WEAPON);
    crate::sfx::Play {
        sound: crate::sfx::Of::Id(id),
        at: None,
        rate,
    }
}

/// Puts the selected set's items into the hero's hands as they change (`sdk_EQUIP`): its arms
/// made anew with them, the hero stopped where the class of what it holds changes (`0x54E6D0`).
fn arm(
    sets: Option<Res<WeaponSets>>,
    maker: Option<Res<crate::arsenal::Maker>>,
    npcs: Option<Res<crate::npcs::Npcs>>,
    mut rearms: MessageWriter<crate::arsenal::Rearm>,
    mut bodies: Query<&mut crate::hero::Body>,
    mut held: Local<Option<([Option<Dropped>; HALVES], u8)>>,
) {
    let (Some(sets), Some(maker), Some(npcs)) = (sets, maker, npcs) else {
        return;
    };
    let hands = sets.halves[sets.selected];
    let items: Vec<(u32, Option<fight::ItemValues>)> = hands
        .iter()
        .flatten()
        .map(|item| (item.item, item.values))
        .collect();
    let Some((before, class)) = *held else {
        // The game's start: the hero holds its first set's items as its template does.
        let (class, _) = maker.arms(&npcs, &items);
        *held = Some((hands, class));
        return;
    };
    if before == hands {
        return;
    }
    let (now, arms) = maker.arms(&npcs, &items);
    info!(
        "sets: the hero's hands take {:?}, of the class {now}",
        items.iter().map(|&(item, _)| item).collect::<Vec<_>>()
    );
    rearms.write(crate::arsenal::Rearm(arms));
    if now != class {
        for mut body in &mut bodies {
            body.stop();
        }
    }
    *held = Some((hands, now));
}

/// The taskbar's weapon sets.
pub struct SetsPlugin;

impl Plugin for SetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), start)
            .add_systems(
                Update,
                (show, lay_out, glow)
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                Update,
                (choose, send)
                    .in_set(crate::frame::Unpaused)
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                Update,
                arm.after(choose)
                    .before(crate::frame::Step::HeroSpawn)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_on_a_slots_left_half_tries_the_right_hand_first() {
        let corner = bar::weapon_slot(0, 1);
        assert_eq!(halves_tried(corner, Vec2::new(340.0, 723.0)), [1, 0]);
        assert_eq!(halves_tried(corner, Vec2::new(380.0, 723.0)), [0, 1]);
    }

    #[test]
    fn a_rules_pieces_go_back_to_their_items() {
        let item = |item| Dropped {
            kind: Kind::Weapon,
            item,
            level: 1,
            key: 16,
            quality: 0,
            amount: 1,
            values: None,
            named: None,
        };
        let halves = [None, Some(item(7901))];
        let sword = rule::Piece::weapon(1, 1, 7901);
        let dagger = rule::Piece::weapon(PUT, 5, 1712);
        // The dagger put in the right hand, the sword it replaced given back.
        let (made, replaced) = rule::put([None, Some(sword)], 1, dagger, false).unwrap();
        assert_eq!(
            halves_of(made, &halves, Some(item(1712))),
            [None, Some(item(1712))]
        );
        assert_eq!(
            replaced.and_then(|piece| item_of(piece, &halves, None)),
            Some(item(7901))
        );
    }

    #[test]
    fn the_models_that_stay_at_their_places_are_not_made_anew() {
        let model = |item: u32, x: f32| Model {
            item,
            at: Vec3::new(x, -40.0, DEPTH),
        };
        let had = [(1, model(7, 30.0)), (2, model(9, 96.0))];
        // Another set chosen: the same models at the same places.
        assert_eq!(
            changed(&had, vec![model(9, 96.0), model(7, 30.0)]),
            (vec![], vec![])
        );
        // An item put into the second set: its old model goes and the new one comes, the first
        // set's stays drawn.
        assert_eq!(
            changed(&had, vec![model(7, 30.0), model(4, 96.0)]),
            (vec![2], vec![model(4, 96.0)])
        );
        // The same item moved within its slot as a weapon joins it in the other hand.
        assert_eq!(
            changed(&had, vec![model(7, 38.0), model(9, 96.0)]),
            (vec![1], vec![model(7, 38.0)])
        );
    }

    #[test]
    fn a_click_chooses_the_shown_slot_under_it() {
        // Two slots shown at the level 2: the first from 262, the second from 328.
        assert_eq!(slot_at(Vec2::new(290.0, 723.0), 2), Some(0));
        assert_eq!(slot_at(Vec2::new(360.0, 723.0), 2), Some(1));
        // Between them, above them, or a slot not shown: none.
        assert_eq!(slot_at(Vec2::new(327.0, 723.0), 2), None);
        assert_eq!(slot_at(Vec2::new(360.0, 680.0), 2), None);
        assert_eq!(slot_at(Vec2::new(360.0, 723.0), 1), Some(0));
    }

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
