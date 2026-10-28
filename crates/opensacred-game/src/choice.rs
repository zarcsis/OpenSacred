//! The campaigns' choices of the hero, as the original's (docs/re/menus.md, "The choice of the
//! hero"): the Underworld campaign's, its pedestals empty until the heroes can be saved and
//! imported (#1129, #1117), and the Ancaria campaign's (#1119): its picture of 12 tiles over its water, the eight classes as 3D figures
//! standing on the pedestals and idling, dark until the cursor is on one or it is chosen, and
//! under them a panel of the hero's name, starting as the class's own, the difficulty, a
//! drop-down list of bronze and silver (#1145), Import, Back and Start Game, which shows a class's
//! description while the cursor is on its figure.
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
use bevy::image::{ImageLoaderSettings, ImageSampler};
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

use bevy_kira_audio::prelude::{AudioChannel, AudioControl};

use opensacred_rules::drops::Difficulty;

use crate::game::{AppState, Class, Data, FONT, Request};
use crate::menu::{self, Screen, Words};
use crate::menu_sound::Introductions;
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

/// What the screen holds: the hero chosen, the figure under the cursor, the name typed, the
/// difficulty chosen and those offered, and whether the difficulties' list is open.
#[derive(Resource, Default)]
struct Choice {
    chosen: Option<Class>,
    hovered: Option<Class>,
    name: String,
    difficulty: Difficulty,
    offered: Vec<Difficulty>,
    listing: bool,
}

/// The difficulties that the list offers a new hero: bronze and silver, as the original's box
/// does with its highest 1 (`0x710190`; docs/re/fight.md, "What opens the levels"), up to one
/// that the command line names above them.
fn offered(named: Option<Difficulty>) -> Vec<Difficulty> {
    let highest = named
        .map_or(0, Difficulty::index)
        .max(Difficulty::Silver.index());
    Difficulty::ALL.into_iter().take(highest + 1).collect()
}

/// The introduction of the class whose figure the cursor rests on: the class, how long the cursor
/// has rested there, in seconds, and whether it has spoken.
#[derive(Resource, Default)]
struct Introduction {
    class: Option<Class>,
    rested: f32,
    spoken: bool,
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
    /// The difficulty chosen, on its plate, and the list of those offered.
    Difficulty,
    List,
    Description,
    Start,
}

/// A button's word, lit while the cursor is on its button and grey while the button is greyed
/// out, as the original's (`0x72B490`, `0x72B700`).
#[derive(Component)]
struct Word {
    lit: bool,
    greyed: bool,
}

/// The material that lays the figures' texture on the interface, which the inventory's grid lays
/// its items' with too (`inventory`).
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
pub(crate) struct Layer {
    #[texture(0)]
    #[sampler(1)]
    pub(crate) figures: Handle<Image>,
}

impl UiMaterial for Layer {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

/// The picture's tiles and water, and the panel's atlases, relative to the asset directory.
#[derive(Resource, Default)]
pub struct Images {
    tiles: Vec<Option<String>>,
    bed: Option<String>,
    reflection: Option<String>,
    frames: Option<String>,
    fields: Option<String>,
}

impl Images {
    /// Finds the screen's textures among the converted ones.
    pub fn find(assets: &std::path::Path) -> Self {
        let tiles: Vec<String> = (1..=12).map(|n| format!("gui_uw_char_{n:02}")).collect();
        let mut names: Vec<&str> = tiles.iter().map(String::as_str).collect();
        names.extend([
            "ui_char_center",
            "ui_char_ref",
            "gui_main_12",
            "gui_main_13",
        ]);
        let mut found = hud::find_textures(assets, &names);
        let fields = found.pop().flatten();
        let frames = found.pop().flatten();
        let reflection = found.pop().flatten();
        let bed = found.pop().flatten();
        Self {
            tiles: found,
            bed,
            reflection,
            frames,
            fields,
        }
    }
}

/// The atlases of the interface that the panel's pieces are cut from.
#[derive(Clone, Copy)]
enum Sheet {
    /// `gui_main_12`: the frame, the rule, the buttons and the difficulty's bar.
    Frames,
    /// `gui_main_13`: the name's field.
    Fields,
}

/// The panel's atlases, loaded to be sampled texel by texel: the original draws its pieces a
/// texel to a pixel, and stretches a button's middle without blurring it.
struct Atlases {
    frames: Option<Handle<Image>>,
    fields: Option<Handle<Image>>,
}

impl Atlases {
    fn load(images: &Images, assets: &AssetServer) -> Self {
        let load = |path: &Option<String>| {
            path.as_ref().map(|path| {
                assets
                    .load_builder()
                    .with_settings(|settings: &mut ImageLoaderSettings| {
                        settings.is_srgb = false;
                        settings.sampler = ImageSampler::nearest();
                    })
                    .load(path.clone())
            })
        };
        Self {
            frames: load(&images.frames),
            fields: load(&images.fields),
        }
    }

    fn sheet(&self, sheet: Sheet) -> Option<&Handle<Image>> {
        match sheet {
            Sheet::Frames => self.frames.as_ref(),
            Sheet::Fields => self.fields.as_ref(),
        }
    }
}

/// An image of the interface's table (set up at `0x761580`): its atlas, its texels from the
/// first to the last (left, top, right, bottom), and whether it is turned about across or along,
/// as the table's entries whose corners are reversed draw their texels (docs/re/menus.md, "The
/// choice of the hero").
#[derive(Clone, Copy)]
struct Piece {
    sheet: Sheet,
    texels: [u16; 4],
    turned: [bool; 2],
}

impl Piece {
    const fn new(sheet: Sheet, texels: [u16; 4]) -> Self {
        Self {
            sheet,
            texels,
            turned: [false, false],
        }
    }

    /// The same texels, turned about across where `across` and along where `along`.
    const fn turned(self, across: bool, along: bool) -> Self {
        Self {
            turned: [across, along],
            ..self
        }
    }

    /// Its size as the original draws it, a texel to a pixel.
    fn size(self) -> Vec2 {
        let [left, top, right, bottom] = self.texels.map(f32::from);
        Vec2::new(right - left + 1.0, bottom - top + 1.0)
    }

    /// Its texels in the atlas, from the first's corner to past the last.
    fn source(self) -> Rect {
        let [left, top, right, bottom] = self.texels.map(f32::from);
        Rect::new(left, top, right + 1.0, bottom + 1.0)
    }
}

/// The interface's images that the panel takes, by their indices in its table: a button's ends
/// and middle (280–282), the rule's middle and ends (294, 296, 297), the frame's sides and edges
/// (310–313), the difficulty's buttons (324, 325) and the name's field's ends and middle (331,
/// 333, 334).
const BUTTON_LEFT: Piece = Piece::new(Sheet::Frames, [1, 39, 45, 75]);
const BUTTON_MIDDLE: Piece = Piece::new(Sheet::Frames, [47, 39, 84, 75]);
const BUTTON_RIGHT: Piece = Piece::new(Sheet::Frames, [88, 39, 133, 75]);
const RULE_MIDDLE: Piece = Piece::new(Sheet::Frames, [173, 1, 204, 17]);
const RULE_LEFT: Piece = Piece::new(Sheet::Frames, [152, 19, 183, 35]);
const RULE_RIGHT: Piece = Piece::new(Sheet::Frames, [185, 19, 216, 35]);
const SIDE_LEFT: Piece = Piece::new(Sheet::Frames, [148, 223, 188, 254]);
const EDGE_TOP: Piece = Piece::new(Sheet::Frames, [190, 178, 221, 221]);
const SIDE_RIGHT: Piece = Piece::new(Sheet::Frames, [150, 223, 173, 254]).turned(true, false);
const EDGE_BOTTOM: Piece = Piece::new(Sheet::Frames, [190, 180, 221, 209]).turned(false, true);
const DROP_LEFT: Piece = Piece::new(Sheet::Frames, [66, 218, 105, 254]);
const DROP_RIGHT: Piece = Piece::new(Sheet::Frames, [66, 180, 105, 216]);
const FIELD_LEFT: Piece = Piece::new(Sheet::Fields, [1, 118, 31, 143]);
const FIELD_MIDDLE: Piece = Piece::new(Sheet::Fields, [33, 118, 63, 143]);
const FIELD_RIGHT: Piece = Piece::new(Sheet::Fields, [66, 118, 97, 143]);
/// The crest's halves at the middle of the frame's top and its foot (314–317), and the frame's
/// corners (318–321): the top left, the top right, the bottom left and the bottom right.
const CREST: [Piece; 4] = [
    Piece::new(Sheet::Frames, [223, 178, 254, 221]),
    Piece::new(Sheet::Frames, [225, 178, 254, 221]).turned(true, false),
    Piece::new(Sheet::Frames, [223, 180, 254, 221]).turned(false, true),
    Piece::new(Sheet::Frames, [225, 180, 254, 221]).turned(true, true),
];
const CORNERS: [Piece; 4] = [
    Piece::new(Sheet::Frames, [148, 178, 188, 221]),
    Piece::new(Sheet::Frames, [150, 178, 188, 221]).turned(true, false),
    Piece::new(Sheet::Frames, [148, 180, 188, 221]).turned(false, true),
    Piece::new(Sheet::Frames, [150, 180, 188, 221]).turned(true, true),
];

/// The panel's dark fill, black at the alpha `0x9F`, and the box its frame stands about, its
/// ornaments outside it (docs/re/menus.md).
const FILL: Rect = Rect::from_corners(Vec2::new(300.0, 563.0), Vec2::new(724.0, 731.0));
const SHADE: Color = Color::linear_rgba(0.0, 0.0, 0.0, 159.0 / 255.0);
const FRAMED: Rect = Rect::from_corners(Vec2::new(320.0, 583.0), Vec2::new(704.0, 711.0));
/// How far below the box the frame's foot stands, and its right side out of its right edge.
const FOOT: f32 = 12.0;
const RIGHT_SIDE: f32 = 15.0;
/// The rule over the buttons: its left end and its width.
const RULE_AT: Vec2 = Vec2::new(300.0, 683.0);
const RULE_WIDTH: f32 = 424.0;
/// The buttons' middles, their ends outside them, and the rows their words stand on: Back, Start
/// Game and Import.
const BACK_BUTTON: (Rect, f32) = (
    Rect::from_corners(Vec2::new(368.0, 698.0), Vec2::new(464.0, 735.0)),
    720.0,
);
const START_BUTTON: (Rect, f32) = (
    Rect::from_corners(Vec2::new(560.0, 698.0), Vec2::new(656.0, 735.0)),
    720.0,
);
const IMPORT_BUTTON: (Rect, f32) = (
    Rect::from_corners(Vec2::new(448.0, 646.0), Vec2::new(576.0, 683.0)),
    668.0,
);
/// The name's field's middle, its ends outside it, and where the name starts and the row it
/// stands on.
const NAME_FIELD: Rect = Rect::from_corners(Vec2::new(380.0, 575.0), Vec2::new(644.0, 599.0));
const NAME_AT: Vec2 = Vec2::new(385.0, 591.0);
/// The difficulty's bar, a button's middle with the two buttons that open the list outside it,
/// and the row the difficulty stands on.
const DIFFICULTY_BAR: Rect = Rect::from_corners(Vec2::new(448.0, 606.0), Vec2::new(576.0, 643.0));
const DIFFICULTY_ROW: f32 = 627.0;
/// The list's sides, the top of its dark box, which dims what lies behind it, the box's height
/// past its rows, the row its first difficulty stands on, and the pitch of the next.
const LIST_SIDES: [f32; 2] = [440.0, 584.0];
const LIST_TOP: f32 = 634.0;
const LIST_BOX: f32 = 8.0;
const LIST_BOX_PITCH: f32 = 20.0;
const LIST_FIRST: f32 = 652.0;
const LIST_PITCH: f32 = 22.0;
const LIST_SHADE: Color =
    Color::linear_rgba(31.0 / 255.0, 31.0 / 255.0, 31.0 / 255.0, 175.0 / 255.0);
/// A difficulty of the list under the cursor: pure yellow, in the frame's values.
const LIST_LIT: Color = Color::linear_rgb(1.0, 1.0, 0.0);
/// Where a description stands: its lines from the column 317 to 706, its first line's letters'
/// foot on the row 580, in the panel's box of 150 rows from the row 562.
const DESCRIPTION: Rect = Rect::from_corners(Vec2::new(317.0, 565.0), Vec2::new(706.0, 712.0));
/// The panel's font, as the original makes it: 17 pixels to the em, a line every 20 pixels, a
/// letter's foot 15 rows under its line's top, and its letters a pixel further apart than the
/// font's advances (docs/re/menus.md).
const TEXT_SIZE: f32 = 17.0;
const LINE: f32 = 20.0;
const FOOT_ROW: f32 = 15.0;
const SPACING: f32 = 1.0;
/// The words' colours, in the frame's sRGB values (`hud`): a button's, lit under the cursor, and
/// greyed out; a description's, and its marked words'.
const TEXT: Color = Color::WHITE;
const LIT: Color = Color::linear_rgb(1.0, 1.0, 128.0 / 255.0);
const TEXT_GREY: Color = Color::linear_rgb(127.0 / 255.0, 127.0 / 255.0, 127.0 / 255.0);
const PLAIN: Color = Color::linear_rgb(198.0 / 255.0, 202.0 / 255.0, 187.0 / 255.0);
const MARKED: Color = LIT;
/// How long the name's caret shows, and then hides, in milliseconds of real time.
const BLINK: u128 = 300;
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
    let named = request.0.difficulty;
    choice.difficulty = named.unwrap_or_default();
    choice.offered = offered(named);
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
            let atlases = Atlases::load(&images, &assets);
            panel(picture, &atlases, &words, &choice.offered, &font);
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
    let atlases = Atlases::load(&images, &assets);
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
            frame(picture, &atlases);
            field(picture, &atlases);
            button(
                picture,
                &atlases,
                (words.get(BACK).unwrap_or("Back"), &font, false),
                BACK_BUTTON,
                |commands| commands.run_system_cached(back),
            );
            button(
                picture,
                &atlases,
                (words.get(START).unwrap_or("Start Game"), &font, true),
                START_BUTTON,
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

/// A piece of the panel's atlases stretched over the picture's rectangle `rect`.
fn stretch(
    parent: &mut ChildSpawnerCommands,
    atlases: &Atlases,
    piece: Piece,
    rect: Rect,
) -> Option<Entity> {
    let sheet = atlases.sheet(piece.sheet)?;
    let [flip_x, flip_y] = piece.turned;
    let image = ImageNode {
        rect: Some(piece.source()),
        flip_x,
        flip_y,
        image_mode: NodeImageMode::Stretch,
        ..ImageNode::new(sheet.clone())
    };
    Some(parent.spawn((image, Pickable::IGNORE, place(rect))).id())
}

/// A piece of the panel's atlases at its own size, its top left corner at `at`.
fn put(parent: &mut ChildSpawnerCommands, atlases: &Atlases, piece: Piece, at: Vec2) {
    stretch(
        parent,
        atlases,
        piece,
        Rect::from_corners(at, at + piece.size()),
    );
}

/// Three pieces in a row, as the original draws its buttons and fields: the middle stretched over
/// the rectangle `rect`, the ends outside its sides, as tall as it and `ends` wide.
fn row(
    parent: &mut ChildSpawnerCommands,
    atlases: &Atlases,
    [left, middle, right]: [Piece; 3],
    rect: Rect,
    ends: [f32; 2],
) {
    stretch(parent, atlases, middle, rect);
    stretch(
        parent,
        atlases,
        left,
        Rect::new(rect.min.x - ends[0], rect.min.y, rect.min.x, rect.max.y),
    );
    stretch(
        parent,
        atlases,
        right,
        Rect::new(rect.max.x, rect.min.y, rect.max.x + ends[1], rect.max.y),
    );
}

/// The node of a line of the panel's words from the column `left` to `right`, its letters' foot
/// on the row `foot`.
fn line(left: f32, right: f32, foot: f32) -> Node {
    let top = foot - FOOT_ROW;
    place(Rect::new(left, top, right, top + LINE))
}

/// The panel's font, its line and the spacing of its letters.
fn lettering(font: &Handle<Font>) -> (TextFont, LineHeight, LetterSpacing) {
    (
        TextFont {
            font: font.clone().into(),
            font_size: FontSize::Px(TEXT_SIZE),
            ..default()
        },
        LineHeight::Px(LINE),
        LetterSpacing::Px(SPACING),
    )
}

/// A button: its pieces about its middle `rect`, its word centred over the middle with its foot
/// on the row `foot`, greyed out where `greyed`, and what a click on it does.
fn button(
    parent: &mut ChildSpawnerCommands,
    atlases: &Atlases,
    (word, font, greyed): (&str, &Handle<Font>, bool),
    (rect, foot): (Rect, f32),
    act: fn(&mut Commands),
) -> Entity {
    row(
        parent,
        atlases,
        [BUTTON_LEFT, BUTTON_MIDDLE, BUTTON_RIGHT],
        rect,
        [BUTTON_LEFT.size().x, BUTTON_RIGHT.size().x],
    );
    let top = foot - FOOT_ROW - rect.min.y;
    parent
        .spawn((
            Word { lit: false, greyed },
            place(rect),
            children![(
                Pickable::IGNORE,
                place(Rect::new(0.0, top, rect.width(), top + LINE)),
                Text::new(word),
                lettering(font),
                TextLayout::justify(Justify::Center),
                TextColor(if greyed { TEXT_GREY } else { TEXT }),
            )],
        ))
        .observe(|over: On<PointerOver>, mut words: Query<&mut Word>| {
            if let Ok(mut word) = words.get_mut(over.entity) {
                word.lit = true;
            }
        })
        .observe(|out: On<PointerOut>, mut words: Query<&mut Word>| {
            if let Ok(mut word) = words.get_mut(out.entity) {
                word.lit = false;
            }
        })
        .observe(move |click: On<PointerClick>, mut commands: Commands| {
            if click.button == PointerButton::Primary {
                act(&mut commands);
            }
        })
        .id()
}

/// Colours the buttons' words as they stand: grey while greyed out, else lit under the cursor.
fn colour_words(
    words: Query<(&Word, &Children), Changed<Word>>,
    mut colours: Query<&mut TextColor>,
) {
    for (word, children) in &words {
        let colour = if word.greyed {
            TEXT_GREY
        } else if word.lit {
            LIT
        } else {
            TEXT
        };
        for &child in children {
            if let Ok(mut text) = colours.get_mut(child) {
                text.0 = colour;
            }
        }
    }
}

/// The name's field: its middle stretched over `NAME_FIELD`, its emblem at the left and its end
/// at the right as wide as its middle, as the original's edit control draws them (`0x7382F0`).
fn field(parent: &mut ChildSpawnerCommands, atlases: &Atlases) {
    row(
        parent,
        atlases,
        [FIELD_LEFT, FIELD_MIDDLE, FIELD_RIGHT],
        NAME_FIELD,
        [FIELD_LEFT.size().x, FIELD_MIDDLE.size().x],
    );
}

/// The panel under the figures, its list offering the difficulties `offered`.
fn panel(
    picture: &mut ChildSpawnerCommands,
    atlases: &Atlases,
    words: &Words,
    offered: &[Difficulty],
    font: &Handle<Font>,
) {
    frame(picture, atlases);
    controls(picture, atlases, words, font);
    list(picture, words, offered, font);
}

/// The name of `difficulty` in the game's texts.
fn difficulty_name(words: &Words, difficulty: Difficulty) -> &str {
    let index = difficulty.index();
    words
        .get(DIFFICULTIES[index])
        .unwrap_or(crate::game::DIFFICULTIES[index])
}

/// The difficulties' list, over the panel: a catch of the clicks elsewhere, which closes it, its
/// dark box, and each difficulty `offered`, yellow under the cursor and chosen by a click.
fn list(
    picture: &mut ChildSpawnerCommands,
    words: &Words,
    offered: &[Difficulty],
    font: &Handle<Font>,
) {
    let rows = f32::from(u8::try_from(offered.len()).unwrap_or(u8::MAX));
    let bottom = LIST_TOP + LIST_BOX + LIST_BOX_PITCH * rows;
    let [left, right] = LIST_SIDES;
    picture
        .spawn((
            Part::List,
            Visibility::Hidden,
            place(Rect::from_corners(Vec2::ZERO, menu::SCREEN)),
        ))
        .observe(|click: On<PointerClick>, mut choice: ResMut<Choice>| {
            if click.button == PointerButton::Primary {
                choice.listing = false;
            }
        })
        .with_children(|list| {
            list.spawn((
                Pickable::IGNORE,
                place(Rect::new(left, LIST_TOP, right, bottom)),
                BackgroundColor(LIST_SHADE),
            ));
            for (row, &difficulty) in offered.iter().enumerate() {
                let foot = LIST_FIRST + LIST_PITCH * f32::from(u8::try_from(row).unwrap_or(0));
                list.spawn((
                    line(left, right, foot),
                    Text::new(difficulty_name(words, difficulty)),
                    lettering(font),
                    TextLayout::justify(Justify::Center),
                    TextColor(TEXT),
                ))
                .observe(
                    |over: On<PointerOver>, mut colours: Query<&mut TextColor>| {
                        if let Ok(mut colour) = colours.get_mut(over.entity) {
                            colour.0 = LIST_LIT;
                        }
                    },
                )
                .observe(|out: On<PointerOut>, mut colours: Query<&mut TextColor>| {
                    if let Ok(mut colour) = colours.get_mut(out.entity) {
                        colour.0 = TEXT;
                    }
                })
                .observe(
                    move |mut click: On<PointerClick>, mut choice: ResMut<Choice>| {
                        if click.button == PointerButton::Primary {
                            choice.difficulty = difficulty;
                            choice.listing = false;
                            click.propagate(false);
                        }
                    },
                );
            }
        });
}

/// The panel's frame, as the original draws it about the box `FRAMED`: its dark fill, the crest's
/// halves at the middle of its top and its foot, its corners, its edges along the top and the
/// foot, as many whole pieces as the box is wide, its sides down to the box's foot, and the rule
/// over the buttons, its middles up to its right end, drawn last over the last of them.
fn frame(picture: &mut ChildSpawnerCommands, atlases: &Atlases) {
    picture.spawn((Pickable::IGNORE, place(FILL), BackgroundColor(SHADE)));
    let Rect { min, max } = FRAMED;
    let top = min.y - CORNERS[0].size().y;
    let middle = FRAMED.center().x;
    for (piece, x, y) in [
        (CREST[0], middle - CREST[0].size().x, top),
        (CREST[1], middle, top),
        (CREST[2], middle - CREST[2].size().x, max.y),
        (CREST[3], middle, max.y),
        (CORNERS[0], min.x - CORNERS[0].size().x, top),
        (CORNERS[1], max.x, top),
        (CORNERS[2], min.x - CORNERS[2].size().x, max.y),
        (CORNERS[3], max.x, max.y),
    ] {
        put(picture, atlases, piece, Vec2::new(x, y));
    }
    let mut x = min.x;
    while x + EDGE_TOP.size().x <= max.x {
        put(picture, atlases, EDGE_TOP, Vec2::new(x, top));
        put(picture, atlases, EDGE_BOTTOM, Vec2::new(x, max.y + FOOT));
        x += EDGE_TOP.size().x;
    }
    let mut y = min.y;
    while y < max.y {
        put(
            picture,
            atlases,
            SIDE_LEFT,
            Vec2::new(min.x - CORNERS[0].size().x, y),
        );
        put(
            picture,
            atlases,
            SIDE_RIGHT,
            Vec2::new(max.x + RIGHT_SIDE, y),
        );
        y += SIDE_LEFT.size().y;
    }
    put(picture, atlases, RULE_LEFT, RULE_AT);
    let end = RULE_AT.x + RULE_WIDTH - RULE_RIGHT.size().x;
    let mut x = RULE_AT.x + RULE_LEFT.size().x;
    while x < end {
        put(picture, atlases, RULE_MIDDLE, Vec2::new(x, RULE_AT.y));
        x += RULE_MIDDLE.size().x;
    }
    put(picture, atlases, RULE_RIGHT, Vec2::new(end, RULE_AT.y));
}

/// The Ancaria campaign's panel's parts: the name and the difficulty, Import, which a description
/// hides, the description, Back and Start Game.
fn controls(
    picture: &mut ChildSpawnerCommands,
    atlases: &Atlases,
    words: &Words,
    font: &Handle<Font>,
) {
    picture
        .spawn((
            Part::Controls,
            Pickable::IGNORE,
            place(Rect::from_corners(Vec2::ZERO, menu::SCREEN)),
        ))
        .with_children(|controls| {
            field(controls, atlases);
            controls.spawn((
                Part::Name,
                Pickable::IGNORE,
                line(NAME_AT.x, NAME_FIELD.max.x, NAME_AT.y),
                Text::new(""),
                lettering(font),
                TextColor(TEXT),
            ));
            // The difficulty's bar, the two buttons that open the list at its ends.
            stretch(controls, atlases, BUTTON_MIDDLE, DIFFICULTY_BAR);
            let Rect { min, max } = DIFFICULTY_BAR;
            for (piece, rect) in [
                (
                    DROP_LEFT,
                    Rect::new(min.x - DROP_LEFT.size().x, min.y, min.x, max.y),
                ),
                (
                    DROP_RIGHT,
                    Rect::new(max.x, min.y, max.x + DROP_RIGHT.size().x, max.y),
                ),
            ] {
                let Some(button) = stretch(controls, atlases, piece, rect) else {
                    continue;
                };
                controls
                    .commands()
                    .entity(button)
                    .insert(Pickable::default())
                    .observe(|mut click: On<PointerClick>, mut choice: ResMut<Choice>| {
                        if click.button == PointerButton::Primary {
                            choice.listing = !choice.listing;
                            click.propagate(false);
                        }
                    });
            }
            controls.spawn((
                Part::Difficulty,
                Pickable::IGNORE,
                line(min.x, max.x, DIFFICULTY_ROW),
                Text::new(difficulty_name(words, Difficulty::default())),
                lettering(font),
                TextLayout::justify(Justify::Center),
                TextColor(TEXT),
            ));
        });
    picture
        .spawn((
            Part::Import,
            Pickable::IGNORE,
            place(Rect::from_corners(Vec2::ZERO, menu::SCREEN)),
        ))
        .with_children(|import| {
            button(
                import,
                atlases,
                (words.get(IMPORT).unwrap_or("Import"), font, true),
                IMPORT_BUTTON,
                |_| {},
            );
        });
    picture.spawn((
        Part::Description,
        Pickable::IGNORE,
        Visibility::Hidden,
        place(DESCRIPTION),
        Text::default(),
        lettering(font),
        TextLayout::justify(Justify::Justified),
        TextColor(PLAIN),
    ));
    button(
        picture,
        atlases,
        (words.get(BACK).unwrap_or("Back"), font, false),
        BACK_BUTTON,
        |commands| commands.run_system_cached(back),
    );
    let start = button(
        picture,
        atlases,
        (words.get(START).unwrap_or("Start Game"), font, true),
        START_BUTTON,
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
    info!(
        "choice: a {} named {}, on {:?}",
        class.name(),
        choice.name,
        choice.difficulty
    );
    request.0.class = Some(class);
    request.0.name = Some(choice.name.clone());
    request.0.difficulty = Some(choice.difficulty);
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

/// The panel as the choice stands: Start Game greyed until a hero is chosen, and a figure's
/// description in place of the controls while the cursor is on it.
fn show(
    choice: Res<Choice>,
    words: Res<Words>,
    mut parts: Query<(&Part, &mut Visibility, Option<&mut Text>, Option<&mut Word>)>,
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
    for (part, mut visibility, text, word) in &mut parts {
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
            // The caret blinks in it (`caret`).
            Part::Name => {}
            Part::Difficulty => {
                if let Some(mut text) = text {
                    difficulty_name(&words, choice.difficulty).clone_into(&mut text.0);
                }
            }
            Part::List => {
                *visibility = if choice.listing && choice.chosen.is_some() && description.is_none()
                {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
            Part::Description => {
                *visibility = if description.is_some() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
            Part::Start => {
                if let Some(mut word) = word {
                    word.greyed = choice.chosen.is_none();
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

/// The name with its caret after it, the caret shown and hidden in turns of `BLINK` of real time,
/// as the original's edit control's.
fn caret(
    time: Res<Time<Real>>,
    choice: Res<Choice>,
    mut shown: Local<Option<bool>>,
    mut parts: Query<(&Part, &mut Text)>,
) {
    let on = (time.elapsed().as_millis() / BLINK).is_multiple_of(2);
    if *shown == Some(on) && !choice.is_changed() {
        return;
    }
    *shown = Some(on);
    for (part, mut text) in &mut parts {
        if *part == Part::Name {
            text.0 = if on {
                format!("{}_", choice.name)
            } else {
                choice.name.clone()
            };
        }
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
                    LineHeight::Px(LINE),
                    LetterSpacing::Px(SPACING),
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
/// the cursor leaving it stops it (the sound event (9, 0x44), `0x693FE0`). The introductions play
/// on a channel of their own, whose stop also drops one still loading (#1144).
fn introduce(
    time: Res<Time<Real>>,
    choice: Res<Choice>,
    mut introduction: ResMut<Introduction>,
    (mut sfx, channel, assets): (
        ResMut<Sfx>,
        Res<AudioChannel<Introductions>>,
        Res<AssetServer>,
    ),
) {
    if choice.hovered != introduction.class {
        if introduction.spoken {
            channel.stop();
        }
        *introduction = Introduction {
            class: choice.hovered,
            ..default()
        };
    }
    let Some(class) = introduction.class else {
        return;
    };
    if introduction.spoken {
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
    channel.stop();
    channel.play(sample).with_volume(sfx::decibels(1.0));
    introduction.spoken = true;
}

/// Stops the introduction playing, or loading, as the screen closes.
fn hush(mut introduction: ResMut<Introduction>, channel: Option<Res<AudioChannel<Introductions>>>) {
    if let Some(channel) = channel {
        channel.stop();
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
                    (keys.in_set(crate::frame::Control), show, caret),
                    // The figures, where the game draws models.
                    (dress, settle, shade, npcs::equip, npcs::bind_worn)
                        .chain()
                        .run_if(resource_exists::<Assets<Gltf>>),
                )
                    .chain()
                    .run_if(in_state(Screen::Heroes)),
            )
            .add_systems(Update, colour_words.after(show))
            .add_systems(
                Update,
                introduce
                    .after(show)
                    .run_if(in_state(Screen::Heroes))
                    .run_if(resource_exists::<Sfx>)
                    .run_if(resource_exists::<AudioChannel<Introductions>>),
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
        assert_eq!(request.difficulty, Some(Difficulty::Bronze));
    }

    #[test]
    fn a_new_hero_is_offered_bronze_and_silver() {
        assert_eq!(offered(None), [Difficulty::Bronze, Difficulty::Silver]);
        // One that the command line names above them is offered with those below it.
        assert_eq!(offered(Some(Difficulty::Gold)).len(), 3);
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
    fn the_frame_lays_its_pieces_where_the_original_does() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let atlases = Atlases {
            frames: Some(Handle::default()),
            fields: Some(Handle::default()),
        };
        app.world_mut()
            .commands()
            .spawn(Node::default())
            .with_children(|picture| frame(picture, &atlases));
        app.world_mut().flush();
        let px = |val: Val| match val {
            Val::Px(px) => px,
            _ => panic!("{val:?}"),
        };
        // Each piece's place, size, first texel and turns.
        let pieces: Vec<([f32; 4], Vec2, [bool; 2])> = app
            .world_mut()
            .query::<(&ImageNode, &Node)>()
            .iter(app.world())
            .map(|(image, node)| {
                (
                    [node.left, node.top, node.width, node.height].map(px),
                    image.rect.map(|rect| rect.min).unwrap_or_default(),
                    [image.flip_x, image.flip_y],
                )
            })
            .collect();
        // The crest's 4, the corners' 4, the edges' 12 and 12, the sides' 4 and 4, and the rule's
        // 14.
        assert_eq!(pieces.len(), 54);
        let at = |place: [f32; 4]| pieces.iter().filter(|p| p.0 == place).count();
        // The top right corner, turned across, and the crest's bottom right half, turned both ways.
        assert!(pieces.contains(&(
            [704.0, 539.0, 39.0, 44.0],
            Vec2::new(150.0, 178.0),
            [true, false]
        )));
        assert!(pieces.contains(&(
            [512.0, 711.0, 30.0, 42.0],
            Vec2::new(225.0, 180.0),
            [true, true]
        )));
        // The foot's edges 12 rows under the box, the right sides 15 columns out of it.
        assert_eq!(at([672.0, 723.0, 32.0, 30.0]), 1);
        for y in [583.0, 615.0, 647.0, 679.0] {
            assert_eq!(at([719.0, y, 24.0, 32.0]), 1, "{y}");
        }
        assert_eq!(at([719.0, 711.0, 24.0, 32.0]), 0);
        // The rule's right end over its last middle.
        assert_eq!(at([684.0, 683.0, 32.0, 17.0]), 1);
        assert_eq!(at([692.0, 683.0, 32.0, 17.0]), 1);
        assert_eq!(at([716.0, 683.0, 32.0, 17.0]), 0);
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
