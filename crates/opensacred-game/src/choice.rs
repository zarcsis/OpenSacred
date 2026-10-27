//! The campaigns' choices of the hero, as the original's (docs/re/menus.md, "The choice of the
//! hero"): the Underworld campaign's, its pedestals empty until the heroes can be saved and
//! imported (#1129, #1117), and the Ancaria campaign's (#1119): its picture of 12 tiles over its water, the eight classes as 3D figures
//! standing on the pedestals and idling, dark until the cursor is on one or it is chosen, and
//! under them a panel of the hero's name, starting as the class's own, the difficulty, Import,
//! Back and Start Game, which shows a class's description while the cursor is on its figure.
//!
//! The figures stand in their own scene ([`FIGURES`]), which the original's camera draws into a
//! texture of its frustum's shape, stretched over the picture as the original's frustum is over
//! the screen; the interface lays that texture between the picture and the panel, in the frame's
//! sRGB values (`figures.wesl`). A figure is its class's model where the client's table puts it
//! (`tables/figures.ron`), in the table's items and its class's default (`npcs::figure_gear`),
//! its idle clip by what it holds, and copies of its materials, which its light darkens or
//! brightens alone; the cursor lights it in its table's window (docs/re/equipment.md, "The
//! choice of the hero's figures"; #1130).
//!
//! The mouse chooses a figure by pointing and clicking, the arrows choose the next or the one
//! before, a key types the name, Enter starts the game and Esc goes back (`menu`). The cursor
//! resting on a figure makes it speak its class's introduction (#1128).

use bevy::asset::embedded_asset;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::picking::events::{PointerClick, PointerOut, PointerOver};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, TextureFormat};
use bevy::shader::ShaderRef;
use bevy::text::{FontSize, LetterSpacing, LineHeight};
use bevy::ui_render::prelude::{MaterialNode, UiMaterial, UiMaterialPlugin};
use bevy::window::PrimaryWindow;

use opensacred_data::tables::{FIGURES as FIGURES_TABLE, Figure as Showcase};

use bevy_kira_audio::prelude::{Audio, AudioControl, AudioInstance, AudioTween};

use crate::game::{AppState, Class, Data, FONT, Request};
use crate::menu::{self, Screen, Words};
use crate::ripples::{self, Pond, Ripples};
use crate::sfx::{self, Sfx};
use crate::{files, hud, motions, npcs};

/// The figures' scene, which the main camera does not see.
const FIGURES: usize = 1;
/// The shader that lays the figures on the interface, embedded in the binary.
const SHADER: &str = "embedded://opensacred_game/figures.wesl";

/// The figures in the screen's order, left to right, as the client's table stands them.
const ORDER: [Class; 8] = [
    Class::Gladiator,
    Class::Seraphim,
    Class::Daemon,
    Class::Dwarf,
    Class::WoodElf,
    Class::Mage,
    Class::DarkElf,
    Class::Vampiress,
];

/// The slots of a hero's that the table's eight items go into (`0x70C710`): the right hand, the
/// left hand, the body, none (the boots, never shown), the head, the hands, the belt and the
/// back.
const SLOTS: [Option<u8>; 8] = [
    Some(13),
    Some(12),
    Some(1),
    None,
    Some(0),
    Some(4),
    Some(2),
    Some(17),
];

/// The figures' camera (`cUI_NetGranny`, `0x6F50B0`): its horizontal angle in degrees, its
/// vertical one a share of it, its near and far planes, and its eye's distance from the
/// figures' origin and its pitch, from below, in radians; it looks at the origin, over the
/// whole screen.
const ANGLE: f32 = 69.7;
const VERTICAL: f32 = 0.75;
const NEAR: f32 = 10.0;
const FAR: f32 = 1000.0;
const DISTANCE: f32 = 100.0;
const PITCH: f32 = -0.059_203_625;

/// The name of each class's text in the game's texts: its own name, which the hero's starts as,
/// and its description (docs/re/menus.md).
fn texts(class: Class) -> (&'static str, &'static str) {
    match class {
        Class::Seraphim => ("UI_DEFAULT_SERAPHIM", "1061"),
        Class::Gladiator => ("UI_DEFAULT_GLADIATOR", "1060"),
        Class::Mage => ("UI_DEFAULT_KAMPFMAGIER", "1064"),
        Class::WoodElf => ("UI_DEFAULT_WALDELFIN", "1063"),
        Class::DarkElf => ("UI_DEFAULT_DUNKELELF", "1062"),
        Class::Daemon => ("UI_DEFAULT_DAEMONIN", "UI_DEFAULT_DAEMONIN_INFO"),
        Class::Dwarf => ("UI_DEFAULT_ZWERG", "UI_DEFAULT_ZWERG_INFO"),
        Class::Vampiress => ("UI_DEFAULT_VAMPIRIN", "1065"),
    }
}

/// The names of the texts this screen shows (`menu::Words`).
pub fn names() -> impl Iterator<Item = &'static str> {
    ORDER
        .iter()
        .flat_map(|&class| {
            let (name, description) = texts(class);
            [name, description]
        })
        .chain([IMPORT, START, BACK, EMPTY])
        .chain(DIFFICULTIES)
}

/// The texts of the panel's buttons and of the first difficulty.
const IMPORT: &str = "UI_IMPORT";
const START: &str = "1029";
const BACK: &str = "UI_NET_BACK";
/// The word of a free pedestal of the Underworld campaign (the saves' `<empty>`, the same text).
const EMPTY: &str = "UI_SAVE_EMPTY";

/// The centres of the Underworld campaign's labels, a pedestal's each, measured on the original's
/// frames: six free pedestals', then the places of the two heroes that the installation ships.
const PEDESTALS: [Vec2; 8] = [
    Vec2::new(94.0, 496.0),
    Vec2::new(220.0, 382.0),
    Vec2::new(360.0, 391.0),
    Vec2::new(527.0, 447.0),
    Vec2::new(637.0, 372.0),
    Vec2::new(716.0, 427.0),
    Vec2::new(807.0, 455.0),
    Vec2::new(905.0, 637.0),
];
const PEDESTAL_LABEL: Vec2 = Vec2::new(120.0, 28.0);
/// The labels' size and their shadow, which keeps them read over the mist.
const PEDESTAL_SIZE: f32 = 21.0;
const SHADOW: Color = Color::linear_rgba(0.0, 0.0, 0.0, 0.8);
/// The difficulties' texts, bronze's first (`0x710190`).
const DIFFICULTIES: [&str; 5] = [
    "UI_DIFFICULTY_BRONZE",
    "UI_DIFFICULTY_SILVER",
    "UI_DIFFICULTY_GOLD",
    "UI_DIFFICULTY_PLATINUM",
    "UI_DIFFICULTY_NIOB",
];

/// The longest name the field takes (**hypothesis**: the driver clears the original's with 24
/// backspaces).
const NAME_LENGTH: usize = 24;

/// What the screen holds: the hero chosen, the figure under the cursor and the name typed.
#[derive(Resource, Default)]
struct Choice {
    chosen: Option<Class>,
    hovered: Option<Class>,
    name: String,
}

/// The introduction of the class whose figure the cursor rests on: the class, how long the cursor
/// has rested there, in seconds, and the introduction playing, once it has.
#[derive(Resource, Default)]
struct Introduction {
    class: Option<Class>,
    rested: f32,
    playing: Option<Handle<AudioInstance>>,
}

/// How long the cursor rests on a figure before it speaks, in seconds (`0x70C710`).
const RESTS: f32 = 0.5;

/// Each class's introduction, a sound id: `SOUND_FX_1060` to `1065`, `SOUND_FX_STARTUP_DEMON`
/// and `SOUND_FX_STARTUP_DWARF` (`0x693FE0`; docs/re/menus.md, "The sound").
fn introduction(class: Class) -> u32 {
    match class {
        Class::Gladiator => 17000,
        Class::Seraphim => 18120,
        Class::DarkElf => 16600,
        Class::WoodElf => 17370,
        Class::Mage => 17750,
        Class::Vampiress => 18510,
        Class::Daemon => 21219,
        Class::Dwarf => 21545,
    }
}

/// The figures, once read.
#[derive(Resource, Default)]
struct Figures(Option<Vec<Look>>);

/// A figure: its class, its window in the picture, where it stands and turns in its scene, and
/// its model, idle clip and items where its model was converted.
struct Look {
    class: Class,
    window: Rect,
    place: Transform,
    model: Option<String>,
    idle: String,
    held: Vec<npcs::Held>,
}

/// A figure, of its class; its glTF until its scene is in place.
#[derive(Component)]
struct Figure {
    class: Class,
    gltf: Option<Handle<Gltf>>,
    idle: String,
}

/// A figure's own copies of its meshes' materials with their colours and reflectances lit, which
/// its light changes alone, and whether it stands lit.
#[derive(Component, Default)]
struct Shades {
    materials: Vec<(Handle<StandardMaterial>, Color, f32)>,
    lit: Option<bool>,
}

/// A mesh of a figure whose material is the figure's own.
#[derive(Component)]
struct Owned;

/// The panel's parts that change.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Part {
    /// The name's field and the difficulty's row, which show once a hero is chosen, and Import,
    /// which a description hides.
    Controls,
    Import,
    Name,
    Description,
    Start,
}

/// The material that lays the figures' texture on the interface.
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
struct Layer {
    #[texture(0)]
    #[sampler(1)]
    figures: Handle<Image>,
}

impl UiMaterial for Layer {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

/// The picture's tiles and water, and the panel's atlas, relative to the asset directory.
#[derive(Resource, Default)]
pub struct Images {
    tiles: Vec<Option<String>>,
    bed: Option<String>,
    reflection: Option<String>,
    atlas: Option<String>,
}

impl Images {
    /// Finds the screen's textures among the converted ones.
    pub fn find(assets: &std::path::Path) -> Self {
        let tiles: Vec<String> = (1..=12).map(|n| format!("gui_uw_char_{n:02}")).collect();
        let mut names: Vec<&str> = tiles.iter().map(String::as_str).collect();
        names.extend(["ui_char_center", "ui_char_ref", "gui_main_12"]);
        let mut found = hud::find_textures(assets, &names);
        let atlas = found.pop().flatten();
        let reflection = found.pop().flatten();
        let bed = found.pop().flatten();
        Self {
            tiles: found,
            bed,
            reflection,
            atlas,
        }
    }
}

/// The pieces of `gui_main_12` that the panel takes, in its pixels: a button with a jewel at each
/// end, the field's bar, the arrows to the left and to the right, and a corner's ornament.
const BUTTON: Rect = Rect::from_corners(Vec2::new(30.0, 144.0), Vec2::new(146.0, 181.0));
const FIELD: Rect = Rect::from_corners(Vec2::new(30.0, 124.0), Vec2::new(128.0, 140.0));
const LEFT: Rect = Rect::from_corners(Vec2::new(110.0, 78.0), Vec2::new(146.0, 114.0));
const RIGHT: Rect = Rect::from_corners(Vec2::new(73.0, 78.0), Vec2::new(109.0, 114.0));
const CORNER: Rect = Rect::from_corners(Vec2::new(148.0, 58.0), Vec2::new(188.0, 98.0));
/// The ornamented strips of the frame's edges: along it, and up and down its sides.
const STRIP: Rect = Rect::from_corners(Vec2::new(152.0, 23.0), Vec2::new(216.0, 32.0));
const SIDE: Rect = Rect::from_corners(Vec2::new(157.0, 145.0), Vec2::new(165.0, 178.0));
/// The ends of a button and of the field, which keep their size as the middle stretches.
const BUTTON_ENDS: f32 = 34.0;
const FIELD_ENDS: f32 = 8.0;

/// The panel's frame in the picture, and its parts' centres and sizes (docs/re/menus.md).
const PANEL: Rect = Rect::from_corners(Vec2::new(285.0, 547.0), Vec2::new(740.0, 748.0));
const NAME_AT: Vec2 = Vec2::new(510.0, 586.0);
const NAME_SIZE: Vec2 = Vec2::new(320.0, 22.0);
const DIFFICULTY_AT: Vec2 = Vec2::new(510.0, 621.0);
const DIFFICULTY_SIZE: Vec2 = Vec2::new(130.0, 22.0);
const ARROWS: [f32; 2] = [428.0, 593.0];
const ARROW: f32 = 28.0;
const IMPORT_AT: Vec2 = Vec2::new(510.0, 663.0);
const IMPORT_SIZE: Vec2 = Vec2::new(210.0, 28.0);
const BACK_AT: Vec2 = Vec2::new(415.0, 714.0);
const START_AT: Vec2 = Vec2::new(607.0, 714.0);
const BUTTON_SIZE: Vec2 = Vec2::new(176.0, 28.0);
/// The row of the strip over the buttons, and where a description stands.
const SEPARATOR: f32 = 690.0;
const DESCRIPTION: Rect = Rect::from_corners(Vec2::new(317.0, 565.0), Vec2::new(705.0, 686.0));
/// The panel's inner border: its light outer line and dark inner one, a pixel each, around the
/// panel's upper part (the original's frames).
const INNER: Rect = Rect::from_corners(Vec2::new(299.0, 562.0), Vec2::new(725.0, 685.0));
const INNER_LIGHT: Color = Color::linear_rgb(119.0 / 255.0, 102.0 / 255.0, 102.0 / 255.0);
const INNER_DARK: Color = Color::linear_rgb(34.0 / 255.0, 17.0 / 255.0, 17.0 / 255.0);
/// How far the corners' ornaments stand out of the panel's sides.
const CORNER_OUT: f32 = 12.0;
/// The panel's fill and its edge, and the texts' sizes and colours, in the frame's sRGB values
/// (`hud`).
const FILL: Color = Color::linear_rgba(0.02, 0.02, 0.03, 0.62);
const EDGE: Color = Color::linear_rgb(0.42, 0.42, 0.45);
const TEXT_SIZE: f32 = 17.0;
const DESCRIPTION_SIZE: f32 = 18.0;
const DESCRIPTION_LINE: f32 = 20.0;
/// The space between a description's letters, which the original's lay wider than the font's.
const DESCRIPTION_SPACING: f32 = 0.8;
const TEXT: Color = Color::linear_rgb(0.93, 0.93, 0.93);
const TEXT_GREY: Color = Color::linear_rgb(0.45, 0.45, 0.45);
const PLAIN: Color = Color::linear_rgb(198.0 / 255.0, 202.0 / 255.0, 187.0 / 255.0);
const MARKED: Color = Color::linear_rgb(1.0, 1.0, 128.0 / 255.0);
/// The figures' light: a figure dark is all but a shape, the light from the top left and the
/// ambient light of their scene.
const DARK: Color = Color::srgb(0.09, 0.10, 0.12);
const LIGHT: f32 = 4000.0;
const AMBIENT: f32 = 600.0;

/// The figure of the arrows' next choice from `chosen`, `step` figures on in the screen's order.
fn step(chosen: Option<Class>, step: isize) -> Class {
    let count = ORDER.len();
    let index = match chosen.and_then(|class| ORDER.iter().position(|&c| c == class)) {
        Some(at) => {
            let signed = isize::try_from(at).unwrap_or(0) + step;
            usize::try_from(signed.rem_euclid(isize::try_from(count).unwrap_or(1))).unwrap_or(0)
        }
        None if step < 0 => count - 1,
        None => 0,
    };
    ORDER[index]
}

/// Chooses `class`: its figure lit, the name its own.
fn choose(choice: &mut Choice, words: &Words, class: Class) {
    choice.chosen = Some(class);
    choice.name = words.get(texts(class).0).unwrap_or(class.name()).to_owned();
}

/// Reads the figures from the client's table and the game's data, once: none where the table
/// was not converted.
fn read_figures(data: Option<Res<Data>>, mut figures: ResMut<Figures>) {
    let Some(data) = data else {
        return;
    };
    if figures.0.is_some() {
        return;
    }
    let shared = &data.0;
    let assets = &shared.assets;
    let path = assets.join("tables").join(FIGURES_TABLE);
    let table: Vec<Showcase> = match files::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))
        .and_then(|text| {
            ron::from_str(&text).map_err(|e| anyhow::anyhow!("parsing {}: {e}", path.display()))
        }) {
        Ok(table) => table,
        Err(e) => {
            warn!("choice: no figures: {e:#}");
            figures.0 = Some(Vec::new());
            return;
        }
    };
    let files = match (npcs::model_files(assets), npcs::texture_files(assets)) {
        (Ok(models), Ok(textures)) => (models, textures),
        (Err(e), _) | (_, Err(e)) => {
            warn!("choice: no figures' models: {e:#}");
            (Default::default(), Default::default())
        }
    };
    figures.0 = Some(
        table
            .iter()
            .filter_map(|figure| {
                let class = *ORDER.iter().find(|c| c.creature() == figure.class)?;
                let slots = slots(figure, class);
                let (held_class, held) =
                    match npcs::figure_gear(&shared.tables, (&files.0, &files.1), &slots) {
                        Ok(gear) => gear,
                        Err(e) => {
                            warn!("choice: {}: no items: {e:#}", class.name());
                            (0, Vec::new())
                        }
                    };
                let (grn, [idle, ..], _) = class.preset();
                let model = files.0.get(grn).cloned();
                let idle = model
                    .as_ref()
                    .and_then(|model| motions::Slots::read(&assets.join(model)).ok())
                    .and_then(|slots| {
                        // The Vampiress and the Dark Elf stand in their fight stance, the rest
                        // as they stand by what they hold (`0x6F4C58`).
                        let stance = matches!(class, Class::Vampiress | Class::DarkElf)
                            .then(|| slots.stance(held_class))
                            .flatten();
                        stance
                            .or_else(|| {
                                slots
                                    .clip(motions::Action::Stand, held_class)
                                    .map(|(name, _)| name)
                            })
                            .map(str::to_owned)
                    })
                    .unwrap_or_else(|| idle.to_owned());
                let [left, top, width, height] = figure.window;
                Some(Look {
                    class,
                    window: Rect::new(left, top, left + width, top + height),
                    place: place_of(figure),
                    model,
                    idle,
                    held,
                })
            })
            .collect(),
    );
}

/// The items of a figure of the table in their slots, and its class's default in its slot where
/// that is empty (`0x6F4E60`).
fn slots(figure: &Showcase, class: Class) -> Vec<(u8, u32)> {
    let mut slots: Vec<(u8, u32)> = SLOTS
        .iter()
        .zip(figure.items)
        .filter(|&(_, item)| item != 0)
        .filter_map(|(&slot, item)| Some((slot?, item)))
        .collect();
    if let Some((slot, item)) = class.default_item()
        && !slots.iter().any(|&(taken, _)| taken == slot)
    {
        slots.push((slot, item));
    }
    slots
}

/// Where a figure of the table stands in the figures' scene, and how it turns: its position of
/// the client's world, Z up, its X to the screen's right and its Y towards the camera, is the
/// scene's (X, Z, Y); its turn of `turn` degrees (`0x6F3E90`) faces its model's front, its +X,
/// towards the camera at 0 and to the screen's left as it grows.
fn place_of(figure: &Showcase) -> Transform {
    let [x, y, z] = figure.position;
    let (sin, cos) = figure.turn.to_radians().sin_cos();
    Transform::from_xyz(x, z, y).with_rotation(crate::hero::facing(Vec3::new(-sin, 0.0, cos)))
}

/// The figures' camera ([`ANGLE`]): where it stands and looks, and its projection, whose aspect
/// is its frustum's shape, the ratio of its angles' tangents; its texture takes that shape, which
/// the layer stretches over the picture as the original's frustum is over its screen.
fn camera() -> (Transform, PerspectiveProjection) {
    let horizontal = ANGLE.to_radians();
    let vertical = horizontal * VERTICAL;
    let (sin, cos) = PITCH.sin_cos();
    (
        Transform::from_xyz(0.0, DISTANCE * sin, DISTANCE * cos).looking_at(Vec3::ZERO, Vec3::Y),
        PerspectiveProjection {
            fov: vertical,
            aspect_ratio: (horizontal / 2.0).tan() / (vertical / 2.0).tan(),
            near: NEAR,
            far: FAR,
            ..default()
        },
    )
}

/// The screen: the picture and its water, the figures' layer, the figures' windows and the
/// panel; the figures' camera, light and models.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources one by one"
)]
fn screen(
    mut commands: Commands,
    images: Res<Images>,
    words: Res<Words>,
    figures: Res<Figures>,
    assets: Res<AssetServer>,
    (window, request): (Option<Single<&Window, With<PrimaryWindow>>>, Res<Request>),
    mut targets: ResMut<Assets<Image>>,
    mut layers: ResMut<Assets<Layer>>,
    mut choice: ResMut<Choice>,
    mut ripples: Ripples,
) {
    *choice = Choice::default();
    let height = menu::picture_height(window.map(|window| window.height()));
    let water = ripples.spawn(
        &mut commands,
        (&mut targets, &assets),
        (Screen::Heroes, &pond(&images, true)),
        height,
    );
    let (eye, projection) = camera();
    let size = Vec2::new(height * projection.aspect_ratio, height)
        .round()
        .as_uvec2();
    let target = targets.add(Image::new_target_texture(
        size.x,
        size.y,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands.spawn((
        DespawnOnExit(Screen::Heroes),
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        RenderTarget::from(target.clone()),
        Projection::Perspective(projection),
        eye,
        Tonemapping::None,
        DebandDither::Disabled,
        AmbientLight {
            color: Color::WHITE,
            brightness: AMBIENT,
            affects_lightmapped_meshes: true,
        },
        RenderLayers::layer(FIGURES),
    ));
    commands.spawn((
        DespawnOnExit(Screen::Heroes),
        DirectionalLight {
            illuminance: LIGHT,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-1.0, 1.2, 1.5).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(FIGURES),
    ));
    let looks = figures.0.as_deref().unwrap_or_default();
    for look in looks {
        let Some(model) = &look.model else {
            continue;
        };
        commands.spawn((
            DespawnOnExit(Screen::Heroes),
            Figure {
                class: look.class,
                gltf: Some(assets.load(model.clone())),
                idle: look.idle.clone(),
            },
            Shades::default(),
            npcs::equipment(&assets, &look.held),
            look.place,
            Visibility::default(),
            RenderLayers::layer(FIGURES),
        ));
    }
    let font: Handle<Font> = assets.load(FONT);
    let layer = layers.add(Layer { figures: target });
    picture(
        &mut commands,
        (Screen::Heroes, &images, &assets),
        water,
        |picture| {
            picture.spawn((
                MaterialNode(layer),
                place(Rect::from_corners(Vec2::ZERO, menu::SCREEN)),
            ));
            for look in looks {
                spotted(picture, look.class, look.window);
            }
            let difficulty = request.0.difficulty.unwrap_or_default().index();
            panel(picture, &images, (&words, difficulty), &font, &assets);
        },
    );
}

/// The Underworld campaign's choice of the hero (#1129): the same picture, each pedestal labelled
/// free, as the original labels the pedestals that no saved hero takes; the panel's empty name,
/// Back, and Start Game greyed, no hero being there to choose.
fn underworld_screen(
    mut commands: Commands,
    images: Res<Images>,
    words: Res<Words>,
    assets: Res<AssetServer>,
    window: Option<Single<&Window, With<PrimaryWindow>>>,
    mut targets: ResMut<Assets<Image>>,
    mut ripples: Ripples,
) {
    let font: Handle<Font> = assets.load(FONT);
    let atlas = images.atlas.as_ref();
    let water = ripples.spawn(
        &mut commands,
        (&mut targets, &assets),
        (Screen::Underworld, &pond(&images, false)),
        menu::picture_height(window.map(|window| window.height())),
    );
    picture(
        &mut commands,
        (Screen::Underworld, &images, &assets),
        water,
        |picture| {
            let empty = words.get(EMPTY).unwrap_or("<empty>");
            for at in PEDESTALS {
                picture.spawn((
                    Pickable::IGNORE,
                    Node {
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..place(about(at, PEDESTAL_LABEL))
                    },
                    children![(
                        Text::new(empty),
                        TextFont {
                            font: font.clone().into(),
                            font_size: FontSize::Px(PEDESTAL_SIZE),
                            ..default()
                        },
                        TextColor(TEXT),
                        TextShadow {
                            offset: Vec2::splat(1.5),
                            color: SHADOW,
                        },
                    )],
                ));
            }
            frame(picture, atlas, &assets);
            piece(
                picture,
                atlas,
                &assets,
                (FIELD, FIELD_ENDS),
                about(NAME_AT, NAME_SIZE),
            );
            button(
                picture,
                (atlas, &assets),
                (words.get(BACK).unwrap_or("Back"), &font, TEXT),
                (BACK_AT, BUTTON_SIZE),
                |commands| commands.run_system_cached(back),
            );
            button(
                picture,
                (atlas, &assets),
                (words.get(START).unwrap_or("Start Game"), &font, TEXT_GREY),
                (START_AT, BUTTON_SIZE),
                |_| {},
            );
        },
    );
}

/// The water of a choice of the hero (docs/re/menus.md, "The water"): the first game's bed, and
/// its reflection where `reflected`, the Ancaria campaign's, under the camera of the caves.
fn pond(images: &Images, reflected: bool) -> Pond<'_> {
    Pond {
        bed: images.bed.as_ref(),
        reflection: images.reflection.as_ref().filter(|_| reflected),
        view: ripples::CAVE,
        cave: true,
    }
}

/// A choice of the hero's picture over its water's texture `water`, black about it in a wider
/// window, which `content` fills.
fn picture(
    commands: &mut Commands,
    (screen, images, assets): (Screen, &Images, &AssetServer),
    water: Handle<Image>,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) {
    commands
        .spawn((
            DespawnOnExit(screen),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::BLACK),
        ))
        .with_children(|root| {
            root.spawn(Node {
                width: Val::Px(menu::SCREEN.x),
                height: Val::Px(menu::SCREEN.y),
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|picture| {
                menu::water(picture, water);
                menu::tiles(picture, &images.tiles, assets);
                content(picture);
            });
        });
}

/// The node of the picture's rectangle `rect`.
fn place(rect: Rect) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(rect.min.x),
        top: Val::Px(rect.min.y),
        width: Val::Px(rect.width()),
        height: Val::Px(rect.height()),
        ..default()
    }
}

/// The rectangle of size `size` about `centre`.
fn about(centre: Vec2, size: Vec2) -> Rect {
    Rect::from_center_size(centre, size)
}

/// A figure's window, the cursor over it lighting the figure of `class` and a click choosing it.
fn spotted(picture: &mut ChildSpawnerCommands, class: Class, window: Rect) {
    picture
        .spawn((place(window),))
        .observe(move |_: On<PointerOver>, mut choice: ResMut<Choice>| {
            choice.hovered = Some(class);
        })
        .observe(move |_: On<PointerOut>, mut choice: ResMut<Choice>| {
            if choice.hovered == Some(class) {
                choice.hovered = None;
            }
        })
        .observe(
            move |click: On<PointerClick>, mut choice: ResMut<Choice>, words: Res<Words>| {
                if click.button == PointerButton::Primary {
                    choose(&mut choice, &words, class);
                }
            },
        );
}

/// A piece of the panel's atlas in the picture's rectangle `rect`, its ends `ends` wide kept
/// as its middle stretches.
fn piece(
    parent: &mut ChildSpawnerCommands,
    atlas: Option<&String>,
    assets: &AssetServer,
    (source, ends): (Rect, f32),
    rect: Rect,
) -> Option<Entity> {
    let atlas = atlas?;
    let mut image = ImageNode::new(hud::image(assets, atlas.clone()));
    image.rect = Some(source);
    image.image_mode = if ends > 0.0 {
        NodeImageMode::Sliced(TextureSlicer {
            border: BorderRect {
                min_inset: Vec2::new(ends, 0.0),
                max_inset: Vec2::new(ends, 0.0),
            },
            center_scale_mode: SliceScaleMode::Stretch,
            sides_scale_mode: SliceScaleMode::Stretch,
            max_corner_scale: 1.0,
        })
    } else {
        NodeImageMode::Stretch
    };
    Some(parent.spawn((image, place(rect))).id())
}

/// A piece of the panel's atlas repeated along the picture's rectangle `rect`.
fn tiled(
    parent: &mut ChildSpawnerCommands,
    atlas: Option<&String>,
    assets: &AssetServer,
    source: Rect,
    rect: Rect,
) {
    let Some(atlas) = atlas else {
        return;
    };
    let mut image = ImageNode::new(hud::image(assets, atlas.clone()));
    image.rect = Some(source);
    image.image_mode = NodeImageMode::Tiled {
        tile_x: rect.width() > rect.height(),
        tile_y: rect.height() >= rect.width(),
        stretch_value: 1.0,
    };
    parent.spawn((image, Pickable::IGNORE, place(rect)));
}

/// A text centred in the picture's rectangle `rect`.
fn label(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    font: &Handle<Font>,
    rect: Rect,
    colour: Color,
) -> Entity {
    parent
        .spawn((
            Node {
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..place(rect)
            },
            children![(
                Text::new(text),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(TEXT_SIZE),
                    ..default()
                },
                TextColor(colour),
            )],
        ))
        .id()
}

/// A button: its piece, its word, and what a click on it does.
fn button(
    parent: &mut ChildSpawnerCommands,
    (atlas, assets): (Option<&String>, &AssetServer),
    (word, font, colour): (&str, &Handle<Font>, Color),
    (centre, size): (Vec2, Vec2),
    act: fn(&mut Commands),
) -> Entity {
    let rect = about(centre, size);
    piece(parent, atlas, assets, (BUTTON, BUTTON_ENDS), rect);
    let label = label(parent, word, font, rect, colour);
    parent.commands().entity(label).observe(
        move |click: On<PointerClick>, mut commands: Commands| {
            if click.button == PointerButton::Primary {
                act(&mut commands);
            }
        },
    );
    label
}

/// The panel under the figures.
fn panel(
    picture: &mut ChildSpawnerCommands,
    images: &Images,
    (words, difficulty): (&Words, usize),
    font: &Handle<Font>,
    assets: &AssetServer,
) {
    let atlas = images.atlas.as_ref();
    frame(picture, atlas, assets);
    controls(picture, atlas, (words, difficulty), font, assets);
}

/// The panel's frame: its fill and edge, its corners' ornaments and its strips along the edges
/// and over the buttons.
fn frame(picture: &mut ChildSpawnerCommands, atlas: Option<&String>, assets: &AssetServer) {
    picture.spawn((
        place(PANEL),
        BackgroundColor(FILL),
        Outline::new(Val::Px(2.0), Val::ZERO, EDGE),
    ));
    for (rect, colour) in [
        (INNER, INNER_LIGHT),
        (
            Rect::from_corners(INNER.min + 1.0, INNER.max - 1.0),
            INNER_DARK,
        ),
    ] {
        picture.spawn((
            Pickable::IGNORE,
            Node {
                border: UiRect::all(Val::Px(1.0)),
                ..place(rect)
            },
            BorderColor::all(colour),
        ));
    }
    // The corners' ornaments, the top left's turned about for the others, standing out of the
    // panel's sides.
    let out = Vec2::new(CORNER_OUT, 0.0);
    for (corner, flip) in [
        (PANEL.min - out, Vec2::ONE),
        (
            Vec2::new(PANEL.max.x - 40.0, PANEL.min.y) + out,
            Vec2::new(-1.0, 1.0),
        ),
        (
            Vec2::new(PANEL.min.x, PANEL.max.y - 40.0) - out,
            Vec2::new(1.0, -1.0),
        ),
        (PANEL.max - 40.0 + out, Vec2::NEG_ONE),
    ] {
        if let Some(corner) = piece(
            picture,
            atlas,
            assets,
            (CORNER, 0.0),
            Rect::from_corners(corner - 6.0, corner + 34.0),
        ) {
            picture
                .commands()
                .entity(corner)
                .insert(UiTransform::from_scale(flip));
        }
    }
    for (source, rect) in [
        (
            STRIP,
            Rect::from_corners(
                PANEL.min - Vec2::new(0.0, 4.0),
                Vec2::new(PANEL.max.x, PANEL.min.y + 5.0),
            ),
        ),
        (
            STRIP,
            Rect::from_corners(
                Vec2::new(PANEL.min.x, PANEL.max.y - 5.0),
                PANEL.max + Vec2::new(0.0, 4.0),
            ),
        ),
        (
            STRIP,
            Rect::from_corners(
                Vec2::new(PANEL.min.x + 12.0, SEPARATOR - 4.0),
                Vec2::new(PANEL.max.x - 12.0, SEPARATOR + 5.0),
            ),
        ),
        (
            SIDE,
            Rect::from_corners(
                PANEL.min - Vec2::new(4.0, 0.0),
                Vec2::new(PANEL.min.x + 4.0, PANEL.max.y),
            ),
        ),
        (
            SIDE,
            Rect::from_corners(
                Vec2::new(PANEL.max.x - 4.0, PANEL.min.y),
                PANEL.max + Vec2::new(4.0, 0.0),
            ),
        ),
    ] {
        tiled(picture, atlas, assets, source, rect);
    }
}

/// The Ancaria campaign's panel's parts: the name, the difficulty and Import, hidden while a
/// description shows, the description, Back and Start Game.
fn controls(
    picture: &mut ChildSpawnerCommands,
    atlas: Option<&String>,
    (words, difficulty): (&Words, usize),
    font: &Handle<Font>,
    assets: &AssetServer,
) {
    picture
        .spawn((
            Part::Controls,
            Pickable::IGNORE,
            place(Rect::from_corners(Vec2::ZERO, menu::SCREEN)),
        ))
        .with_children(|controls| {
            let field = about(NAME_AT, NAME_SIZE);
            piece(controls, atlas, assets, (FIELD, FIELD_ENDS), field);
            controls.spawn((
                Part::Name,
                Node {
                    padding: UiRect::left(Val::Px(12.0)),
                    align_items: AlignItems::Center,
                    ..place(field)
                },
                Text::new(""),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(TEXT_SIZE),
                    ..default()
                },
                TextColor(TEXT),
            ));
            let row = about(DIFFICULTY_AT, DIFFICULTY_SIZE);
            piece(controls, atlas, assets, (FIELD, FIELD_ENDS), row);
            // A new hero's difficulty is bronze alone, the arrows doing nothing, as the original
            // offers it (`0x710190` with 0, docs/re/fight.md, "The difficulty"); one that the
            // command line names stands in its place.
            label(
                controls,
                words
                    .get(DIFFICULTIES[difficulty])
                    .unwrap_or(crate::game::DIFFICULTIES[difficulty]),
                font,
                row,
                TEXT,
            );
            for (x, arrow) in ARROWS.into_iter().zip([LEFT, RIGHT]) {
                piece(
                    controls,
                    atlas,
                    assets,
                    (arrow, 0.0),
                    about(Vec2::new(x, DIFFICULTY_AT.y), Vec2::splat(ARROW)),
                );
            }
        });
    picture
        .spawn((
            Part::Import,
            Pickable::IGNORE,
            place(Rect::from_corners(Vec2::ZERO, menu::SCREEN)),
        ))
        .with_children(|import| {
            let rect = about(IMPORT_AT, IMPORT_SIZE);
            piece(import, atlas, assets, (BUTTON, BUTTON_ENDS), rect);
            label(
                import,
                words.get(IMPORT).unwrap_or("Import"),
                font,
                rect,
                TEXT_GREY,
            );
        });
    picture.spawn((
        Part::Description,
        Pickable::IGNORE,
        Visibility::Hidden,
        place(DESCRIPTION),
        Text::default(),
        TextFont {
            font: font.clone().into(),
            font_size: FontSize::Px(DESCRIPTION_SIZE),
            ..default()
        },
        LineHeight::Px(DESCRIPTION_LINE),
        LetterSpacing::Px(DESCRIPTION_SPACING),
        TextLayout::justify(Justify::Justified),
        TextColor(PLAIN),
    ));
    button(
        picture,
        (atlas, assets),
        (words.get(BACK).unwrap_or("Back"), font, TEXT),
        (BACK_AT, BUTTON_SIZE),
        |commands| commands.run_system_cached(back),
    );
    let start = button(
        picture,
        (atlas, assets),
        (words.get(START).unwrap_or("Start Game"), font, TEXT_GREY),
        (START_AT, BUTTON_SIZE),
        |commands| commands.run_system_cached(start),
    );
    picture.commands().entity(start).insert(Part::Start);
}

/// Back to the main menu.
fn back(mut screen: ResMut<NextState<Screen>>) {
    screen.set(Screen::Main);
}

/// Starts a game of the hero chosen, of its name, if one is.
fn start(
    choice: Res<Choice>,
    mut request: ResMut<Request>,
    mut state: ResMut<NextState<AppState>>,
) {
    let Some(class) = choice.chosen else {
        return;
    };
    info!("choice: a {} named {}", class.name(), choice.name);
    request.0.class = Some(class);
    request.0.name = Some(choice.name.clone());
    state.set(AppState::Loading);
}

/// The arrows choose the next figure or the one before, a key types the chosen hero's name and
/// Backspace takes its last character back, and Enter starts the game.
fn keys(
    mut commands: Commands,
    mut input: MessageReader<KeyboardInput>,
    mut choice: ResMut<Choice>,
    words: Res<Words>,
) {
    for input in input.read() {
        if !input.state.is_pressed() {
            continue;
        }
        match &input.logical_key {
            Key::ArrowRight | Key::ArrowDown => {
                let class = step(choice.chosen, 1);
                choose(&mut choice, &words, class);
            }
            Key::ArrowLeft | Key::ArrowUp => {
                let class = step(choice.chosen, -1);
                choose(&mut choice, &words, class);
            }
            Key::Enter => commands.run_system_cached(start),
            Key::Backspace if choice.chosen.is_some() => {
                choice.name.pop();
            }
            _ => {
                if choice.chosen.is_some()
                    && let Some(text) = &input.text
                {
                    for character in text.chars().filter(|c| !c.is_control()) {
                        if choice.name.chars().count() < NAME_LENGTH {
                            choice.name.push(character);
                        }
                    }
                }
            }
        }
    }
}

/// The panel as the choice stands: the name with the cursor after it, Start Game greyed until a
/// hero is chosen, and a figure's description in place of the controls while the cursor is on it.
fn show(
    choice: Res<Choice>,
    words: Res<Words>,
    mut parts: Query<(&Part, &mut Visibility, Option<&mut Text>, Option<&Children>)>,
    mut colours: Query<&mut TextColor>,
    mut commands: Commands,
) {
    if !choice.is_changed() {
        return;
    }
    // The chosen hero's own figure shows no description, as the original's.
    let description = choice
        .hovered
        .filter(|&class| choice.chosen != Some(class))
        .map(|class| texts(class).1);
    for (part, mut visibility, text, children) in &mut parts {
        match part {
            Part::Controls => {
                *visibility = if description.is_some() || choice.chosen.is_none() {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                };
            }
            Part::Import => {
                *visibility = if description.is_some() {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                };
            }
            Part::Name => {
                if let Some(mut text) = text {
                    text.0 = format!("{}_", choice.name);
                }
            }
            Part::Description => {
                *visibility = if description.is_some() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
            Part::Start => {
                for child in children.into_iter().flatten() {
                    if let Ok(mut colour) = colours.get_mut(*child) {
                        colour.0 = if choice.chosen.is_some() {
                            TEXT
                        } else {
                            TEXT_GREY
                        };
                    }
                }
            }
        }
    }
    // A description's words, those its marks single out in their colour.
    if let Some(name) = description {
        let text = words.get(name).unwrap_or_default();
        commands.run_system_cached_with(describe, spans(text));
    }
}

/// Writes a description's spans into its text.
fn describe(
    In(spans): In<Vec<(String, bool)>>,
    mut commands: Commands,
    descriptions: Query<(Entity, &Part, &TextFont, Option<&Children>)>,
) {
    for (entity, part, font, children) in &descriptions {
        if *part != Part::Description {
            continue;
        }
        for child in children.into_iter().flatten() {
            commands.entity(*child).despawn();
        }
        let mut node = commands.entity(entity);
        node.insert(Text::default());
        node.with_children(|text| {
            for (words, marked) in spans.iter().cloned() {
                text.spawn((
                    TextSpan::new(words),
                    font.clone(),
                    LineHeight::Px(DESCRIPTION_LINE),
                    LetterSpacing::Px(DESCRIPTION_SPACING),
                    TextColor(if marked { MARKED } else { PLAIN }),
                ));
            }
        });
    }
}

/// A text's spans, each marked where a `<colorw=…>` before it singles its first word out, the
/// other tags left out.
fn spans(text: &str) -> Vec<(String, bool)> {
    let mut spans = Vec::new();
    let mut rest = text;
    let mut marked = false;
    while !rest.is_empty() {
        let Some(open) = rest.find('<') else {
            push(&mut spans, rest, marked);
            break;
        };
        push(&mut spans, &rest[..open], marked);
        let Some(close) = rest[open..].find('>') else {
            break;
        };
        marked = rest[open..open + close].starts_with("<colorw=");
        rest = &rest[open + close + 1..];
    }
    spans
}

/// Pushes `text`, its first word marked where `marked`.
fn push(spans: &mut Vec<(String, bool)>, text: &str, marked: bool) {
    if text.is_empty() {
        return;
    }
    if marked {
        let end = text.find(char::is_whitespace).unwrap_or(text.len());
        spans.push((text[..end].to_owned(), true));
        if end < text.len() {
            spans.push((text[end..].to_owned(), false));
        }
    } else {
        spans.push((text.to_owned(), false));
    }
}

/// Gives a figure its model's scene once its glTF is loaded.
fn dress(
    mut commands: Commands,
    mut figures: Query<(Entity, &mut Figure)>,
    gltfs: Res<Assets<Gltf>>,
) {
    for (entity, mut figure) in &mut figures {
        let Some(gltf) = figure.gltf.as_ref().and_then(|handle| gltfs.get(handle)) else {
            continue;
        };
        if let Some(scene) = gltf.scenes.first().cloned() {
            commands.entity(entity).insert(WorldAssetRoot(scene));
        }
        let idle = gltf.named_animations.get(figure.idle.as_str()).cloned();
        figure.gltf = None;
        if let Some(idle) = idle {
            commands.entity(entity).insert(Idle(idle));
        }
    }
}

/// A figure's idle clip, which its animation player plays once it is made.
#[derive(Component)]
struct Idle(Handle<AnimationClip>);

/// Puts the figures' scene's entities into the figures' scene, plays a figure's idle clip as its
/// player comes, and gives its meshes copies of their materials of its own.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its queries and resources one by one"
)]
fn settle(
    mut commands: Commands,
    mut figures: Query<(Entity, Option<&Idle>, &mut Shades), With<Figure>>,
    children: Query<&Children>,
    layered: Query<(), With<RenderLayers>>,
    mut players: Query<&mut AnimationPlayer, Without<AnimationGraphHandle>>,
    meshes: Query<&MeshMaterial3d<StandardMaterial>, Without<Owned>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    for (figure, idle, mut shades) in &mut figures {
        for entity in children.iter_descendants(figure) {
            if !layered.contains(entity) {
                commands.entity(entity).insert(RenderLayers::layer(FIGURES));
            }
            if let (Some(idle), Ok(mut player)) = (idle, players.get_mut(entity)) {
                let (graph, node) = AnimationGraph::from_clip(idle.0.clone());
                player.play(node).repeat();
                commands
                    .entity(entity)
                    .insert(AnimationGraphHandle(graphs.add(graph)));
            }
            if let Ok(material) = meshes.get(entity)
                && let Some(own) = materials.get(&material.0).cloned()
            {
                let (colour, reflectance) = (own.base_color, own.reflectance);
                let own = materials.add(own);
                shades.materials.push((own.clone(), colour, reflectance));
                shades.lit = None;
                commands.entity(entity).insert((MeshMaterial3d(own), Owned));
            }
        }
    }
}

/// The class of the figure that the cursor rests on speaks its introduction once the cursor has
/// rested there half a second, at the effects' volume, in the middle and at its sample's rate;
/// the cursor leaving it stops it (the sound event (9, 0x44), `0x693FE0`).
fn introduce(
    time: Res<Time<Real>>,
    choice: Res<Choice>,
    mut introduction: ResMut<Introduction>,
    (mut sfx, audio, assets): (ResMut<Sfx>, Res<Audio>, Res<AssetServer>),
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    if choice.hovered != introduction.class {
        if let Some(playing) = introduction.playing.take()
            && let Some(mut instance) = instances.get_mut(&playing)
        {
            instance.stop(AudioTween::default());
        }
        *introduction = Introduction {
            class: choice.hovered,
            ..default()
        };
    }
    let Some(class) = introduction.class else {
        return;
    };
    if introduction.playing.is_some() {
        return;
    }
    introduction.rested += time.delta_secs();
    if introduction.rested <= RESTS {
        return;
    }
    let Some((id, file)) = sfx.with_id(self::introduction(class)) else {
        return;
    };
    let sample = sfx.sample(id, file, &assets);
    debug!("choice: {} introduces itself, {id}", class.name());
    introduction.playing = Some(audio.play(sample).with_volume(sfx::decibels(1.0)).handle());
}

/// Stops the introduction playing as the screen closes.
fn hush(mut introduction: ResMut<Introduction>, instances: Option<ResMut<Assets<AudioInstance>>>) {
    if let Some(playing) = introduction.playing.take()
        && let Some(mut instances) = instances
        && let Some(mut instance) = instances.get_mut(&playing)
    {
        instance.stop(AudioTween::default());
    }
    *introduction = Introduction::default();
}

/// Lights the figure under the cursor and the one chosen, the others dark.
fn shade(
    choice: Res<Choice>,
    mut figures: Query<(&Figure, &mut Shades)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (figure, mut shades) in &mut figures {
        let lit = choice.hovered == Some(figure.class) || choice.chosen == Some(figure.class);
        if shades.lit == Some(lit) {
            continue;
        }
        shades.lit = Some(lit);
        // A dark figure reflects no light either, which would leave it grey.
        for (handle, colour, reflectance) in &shades.materials {
            if let Some(mut material) = materials.get_mut(handle) {
                (material.base_color, material.reflectance) = if lit {
                    (*colour, *reflectance)
                } else {
                    (DARK, 0.0)
                };
            }
        }
    }
}

/// The Ancaria campaign's choice of the hero.
pub struct ChoicePlugin;

impl Plugin for ChoicePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "figures.wesl");
        app.add_plugins(UiMaterialPlugin::<Layer>::default())
            .init_resource::<Choice>()
            .init_resource::<Introduction>()
            .add_systems(OnExit(Screen::Heroes), hush)
            .init_resource::<Figures>()
            .init_resource::<Images>()
            .add_systems(OnEnter(Screen::Heroes), (read_figures, screen).chain())
            .add_systems(OnEnter(Screen::Underworld), underworld_screen)
            .add_systems(
                Update,
                (
                    (keys.in_set(crate::frame::Control), show),
                    // The figures, where the game draws models.
                    (dress, settle, shade, npcs::equip, npcs::bind_worn)
                        .chain()
                        .run_if(resource_exists::<Assets<Gltf>>),
                )
                    .chain()
                    .run_if(in_state(Screen::Heroes)),
            )
            .add_systems(
                Update,
                introduce
                    .after(show)
                    .run_if(in_state(Screen::Heroes))
                    .run_if(resource_exists::<Sfx>)
                    .run_if(resource_exists::<Assets<AudioInstance>>),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arrows_choose_the_figures_in_the_screens_order() {
        assert_eq!(step(None, 1), Class::Gladiator);
        assert_eq!(step(None, -1), Class::Vampiress);
        assert_eq!(step(Some(Class::Gladiator), 1), Class::Seraphim);
        assert_eq!(step(Some(Class::Gladiator), -1), Class::Vampiress);
        assert_eq!(step(Some(Class::Vampiress), 1), Class::Gladiator);
        assert_eq!(step(Some(Class::Dwarf), 1), Class::WoodElf);
    }

    #[test]
    fn the_arrows_choose_a_hero_whose_name_the_keys_type_and_enter_starts_it() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::state::app::StatesPlugin,
            AssetPlugin::default(),
        ))
        .init_asset::<Font>()
        .init_asset::<Image>()
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<bevy::shader::Shader>()
        .init_state::<AppState>()
        .add_message::<KeyboardInput>()
        .add_plugins((menu::MenuPlugin, ChoicePlugin))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<Request>();
        app.update();
        app.world_mut()
            .resource_mut::<NextState<Screen>>()
            .set(Screen::Heroes);
        app.update();
        let typed = |app: &mut App, key: Key, text: Option<&str>| {
            app.world_mut().write_message(KeyboardInput {
                key_code: KeyCode::Unidentified(bevy::input::keyboard::NativeKeyCode::Unidentified),
                logical_key: key,
                state: bevy::input::ButtonState::Pressed,
                text: text.map(Into::into),
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
            app.update();
        };
        // Enter starts nothing before a hero is chosen.
        typed(&mut app, Key::Enter, None);
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Menu
        );
        // Left of no choice is the last figure, the Vampiress; right twice, the Seraphim.
        typed(&mut app, Key::ArrowLeft, None);
        typed(&mut app, Key::ArrowRight, None);
        typed(&mut app, Key::ArrowRight, None);
        typed(&mut app, Key::Character("x".into()), Some("x"));
        typed(&mut app, Key::Character("y".into()), Some("y"));
        typed(&mut app, Key::Backspace, None);
        typed(&mut app, Key::Enter, None);
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Loading
        );
        let request = &app.world().resource::<Request>().0;
        assert_eq!(request.class, Some(Class::Seraphim));
        assert_eq!(request.name.as_deref(), Some("Seraphimx"));
    }

    /// The Gladiator of the client's table, as `tables/figures.ron` holds him.
    fn gladiator() -> Showcase {
        Showcase {
            class: 2,
            items: [1726, 0, 4054, 0, 0, 0, 0, 0],
            position: [-100.0, -82.5, -59.0],
            turn: -30.0,
            floats_34: [-80.0, 90.0, 40.0],
            window: [35.0, 320.0, 120.0, 320.0],
        }
    }

    #[test]
    fn a_figure_takes_its_class_default_where_its_slot_is_empty() {
        // The Gladiator's belt in the body's slot and, his default, in the belt's.
        assert_eq!(
            slots(&gladiator(), Class::Gladiator),
            [(13, 1726), (1, 4054), (2, 4054)]
        );
        // The Dwarf's helmet takes the head's slot, which his goggles do not; his boots go in
        // none.
        let dwarf = Showcase {
            class: 8,
            items: [1853, 0, 3009, 3010, 3012, 3011, 3008, 0],
            ..gladiator()
        };
        assert_eq!(
            slots(&dwarf, Class::Dwarf),
            [(13, 1853), (1, 3009), (0, 3012), (4, 3011), (2, 3008)]
        );
    }

    #[test]
    fn the_camera_shows_a_figure_where_the_original_does() {
        use bevy::camera::CameraProjection;
        let (eye, projection) = camera();
        let place = place_of(&gladiator());
        // The Gladiator's feet, where the original's view and projection (`0x6F50B0`), worked
        // out by hand, put them on its 1024 × 768 screen.
        let clip = projection.get_clip_from_view()
            * eye.to_matrix().inverse()
            * place.translation.extend(1.0);
        let ndc = clip.truncate() / clip.w;
        let feet = Vec2::new(
            (ndc.x + 1.0) / 2.0 * menu::SCREEN.x,
            (1.0 - ndc.y) / 2.0 * menu::SCREEN.y,
        );
        assert!(feet.distance(Vec2::new(100.9, 663.1)) < 0.5, "{feet}");
        // His front turned 30° from the camera to the screen's right.
        let front = place.rotation * Vec3::X;
        assert!(front.distance(Vec3::new(0.5, 0.0, 0.866)) < 1e-3, "{front}");
    }

    #[test]
    fn a_description_singles_out_the_words_its_marks_name() {
        assert_eq!(
            spans("The <colorw=FFFFFF80>Dark <colorw=FFFFFF80>Elf comes <sigh>here"),
            [
                ("The ".to_owned(), false),
                ("Dark".to_owned(), true),
                (" ".to_owned(), false),
                ("Elf".to_owned(), true),
                (" comes ".to_owned(), false),
                ("here".to_owned(), false),
            ]
        );
        assert_eq!(spans("plain"), [("plain".to_owned(), false)]);
    }
}
