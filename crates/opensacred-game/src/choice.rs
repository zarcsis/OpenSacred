//! The campaigns' choices of the hero, as the original's (docs/re/menus.md, "The choice of the
//! hero"): the Underworld campaign's, the heroes that the installation ships standing on their
//! pedestals, each labelled with its name and level, the others free (#1129, #1676), and the
//! Ancaria campaign's (#1119): its picture of 12 tiles over its water, the eight classes as 3D figures
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
//!
//! Both choices stand their figures on the same eight pedestals, the table's places: the Ancaria
//! campaign's classes in the table's order, the Underworld campaign's heroes each on the pedestal
//! of its slot (`0x70C7E0`), in the items of its summary's equipment (`0x6F4E60`). A choice is of
//! a pedestal.

use bevy::asset::embedded_asset;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::ecs::system::EntityCommands;
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::picking::events::{PointerClick, PointerOut, PointerOver};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, TextureFormat};
use bevy::shader::ShaderRef;
use bevy::text::{FontSize, LetterSpacing, LineBreak, LineHeight};
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::window::PrimaryWindow;

use opensacred_data::tables::{FIGURES as FIGURES_TABLE, Figure as Showcase};

use bevy_kira_audio::prelude::{AudioChannel, AudioControl};

use opensacred_rules::drops::Difficulty;

use crate::bar::{self, UiImages};
use crate::game::{AppState, Class, Data, FONT, Request};
use crate::menu::{self, MENU_FONT, Screen, Words};
use crate::menu_sound::Introductions;
use crate::pause::Exporting;
use crate::ripples::{self, Pond, Ripples};
use crate::sfx::{self, Sfx};
use crate::{files, hud, model_effects, motions, npcs};

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
        .chain([
            IMPORT,
            NEW_HERO,
            START,
            BACK,
            EMPTY,
            LEVEL,
            ERASE_CONFIRM,
            MINIMUM,
        ])
        .chain([EXPORT, EXPORT_TITLE, OVERWRITE, ERASE_TIP])
        .chain(DIFFICULTIES)
}

/// The texts of the panel's buttons and of the first difficulty.
const IMPORT: &str = "UI_IMPORT";
/// The import's button back to the classes (`0x70D25C`'s relabelling of Import).
const NEW_HERO: &str = "UI_NET_PLAYER_NEW";
/// The erase button's question (`0x70EE60`) and the title of the Underworld campaign's level
/// (`0x70F450`).
const ERASE_CONFIRM: &str = "UI_CHAR_ERASE_CONFIRM";
/// The erase button's tooltip (`0x7111AB`, its making's `0x7327A0`).
const ERASE_TIP: &str = "UI_CHAR_ERASE_TOOLTIP";
const MINIMUM: &str = "UI_ADDON_MINREQ";
/// The export's button on Start Game's (`+0x398`), its title (`0x70F450`) and its question on a
/// taken pedestal (`0x70D71E`).
const EXPORT: &str = "UI_EXPORT";
const EXPORT_TITLE: &str = "UI_CHAR_EXPORT_LOKAL";
const OVERWRITE: &str = "UI_MSG_OVERWRITE_CHAR";
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
/// A saved hero's label (`0x6F15A0`): its name over `1070`'s "Level" and its level, in the font
/// 2, Carolingia 12 (`world_map`'s size of it), in its class's colour (`0x43C250`), each line
/// centred, a line every 19 pixels, the level's followed by the medal of the highest difficulty
/// the hero won (the interface's images 414 to 418); over a box as wide as its figure's window,
/// two lines and 5 pixels high, its foot 5 under the window's: black at the alpha `0x9F`, dark
/// blue (`0x7F000068`) while the hero is chosen.
const LEVEL: &str = "1070";
const LABEL_SIZE: f32 = 18.0;
const LABEL_LINE: f32 = 19.0;
const LABEL_FOOT: f32 = 5.0;
const MEDALS: u32 = 414;
const MEDAL: f32 = 16.0;
const CHOSEN_BOX: Color = Color::linear_rgba(0.0, 0.0, 104.0 / 255.0, 127.0 / 255.0);
/// The lowest level of a saved hero that the Underworld campaign's choice takes (`0x70CA30`:
/// above 24).
const UNDERWORLD_LEVEL: u32 = 25;
/// The title of that level (`0x70F450`): while the cursor is on a saved hero below it, across the
/// screen's width from the row 92, 16 high, centred, in the font 3, AntiquaSSK 16, at the size
/// the overview writes it (`overview`), red.
const MINIMUM_TITLE: Rect = Rect::from_corners(Vec2::new(0.0, 92.0), Vec2::new(1024.0, 108.0));
const MINIMUM_SIZE: f32 = 24.0;
const MINIMUM_COLOUR: u32 = 0xFFFF_0000;
/// The export's title, in the same place, pale yellow (`0x70F450`: `-0x80`).
const EXPORT_COLOUR: u32 = 0xFFFF_FF80;
/// The erase button (`+0x3B4`, `0x710270`): the interface's image 129, a red cross, 168 right of
/// the middle of the panel's framed box and 5 over its top.
const ERASE: u32 = 129;
const ERASE_AT: Vec2 = Vec2::new(680.0, 578.0);
/// Half the side of its image, 16 by 16, from its corner to its middle, and how far over its top
/// its tooltip's middle stands.
const ERASE_HALF: f32 = 8.0;
const TIP_ABOVE: f32 = 20.0;
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

/// What the screen holds: what stands on each pedestal, the pedestal chosen and the one under
/// the cursor, the name typed, the difficulty chosen and those offered, and whether the
/// difficulties' list is open.
#[derive(Resource, Default)]
struct Choice {
    stands: [Option<Stand>; 8],
    chosen: Option<usize>,
    hovered: Option<usize>,
    name: String,
    difficulty: Difficulty,
    offered: Vec<Difficulty>,
    listing: bool,
    /// Whether the choice is the Ancaria campaign's import, which takes a saved hero of any
    /// level into the Ancaria campaign (#2552), not the Underworld campaign's.
    importing: bool,
    /// Whether the choice is the export's over a game, which takes any pedestal (#2554).
    exporting: bool,
    /// What the screen goes with, once one is made.
    scope: Option<Scope>,
}

/// What a choice's screen goes with: a menu's screen, or the export over a game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scope {
    Menu(Screen),
    Export,
}

impl Scope {
    /// `entity` gone as the screen goes.
    fn bind(self, entity: &mut EntityCommands) {
        match self {
            Self::Menu(screen) => entity.insert(DespawnOnExit(screen)),
            Self::Export => entity.insert(DespawnOnExit(Exporting::Shown)),
        };
    }

    /// The layer of the screen's root, over the game's interface for the export and none in the
    /// menus, and its veil's, over it.
    fn layers(self) -> (Option<i32>, i32) {
        match self {
            Self::Menu(_) => (None, VEIL_LAYER),
            Self::Export => (Some(EXPORT_LAYER), EXPORT_LAYER + 1),
        }
    }
}

/// The export's layer over a game: the saves' and the options' windows'.
const EXPORT_LAYER: i32 = 15;

impl Choice {
    /// What stands on `pedestal`, none on a free one.
    fn stand(&self, pedestal: usize) -> Option<&Stand> {
        self.stands.get(pedestal)?.as_ref()
    }

    /// The class of the Ancaria campaign's figure on `pedestal`, none where a saved hero or
    /// nothing stands there.
    fn class_at(&self, pedestal: Option<usize>) -> Option<Class> {
        self.stand(pedestal?)
            .filter(|stand| stand.saved.is_none())
            .map(|stand| stand.class)
    }
}

/// What stands on a pedestal: a class's figure, the Ancaria campaign's, or a saved hero, the
/// Underworld campaign's.
#[derive(Clone)]
struct Stand {
    class: Class,
    saved: Option<Saved>,
}

/// A saved hero: its name and level, the highest difficulty it won as `0x6F4F20` reads its
/// summary, bronze's where it won none, and its slot where it is one the player exported
/// (`exports`, #2552), none for the installation's.
#[derive(Clone)]
struct Saved {
    name: String,
    level: u32,
    highest: usize,
    exported: Option<u8>,
}

/// The highest difficulty that a saved hero won (`0x6F4F20`): the last of its summary's bytes
/// that is set, bronze's where none is.
fn highest_won(won: &[bool]) -> usize {
    won.iter().rposition(|&won| won).unwrap_or(0)
}

/// The difficulties that the list offers a saved hero: bronze up to one past the highest it
/// won (`0x70CA30`), so silver at least, and niobium at most (`0x710190`).
fn offered_to(highest: usize) -> Vec<Difficulty> {
    Difficulty::ALL.into_iter().take(highest + 2).collect()
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
struct Figures(Option<Lineup>);

/// The figures of the two choices, and the pedestals' windows in the picture, the table's.
#[derive(Default)]
struct Lineup {
    classes: Vec<Look>,
    saved: Vec<Look>,
    windows: Vec<Rect>,
}

/// A figure: what it is, its pedestal and its window in the picture, where it stands and turns
/// in its scene, and its model, idle clip and items where its model was converted.
struct Look {
    stand: Stand,
    pedestal: usize,
    window: Rect,
    place: Transform,
    model: Option<String>,
    idle: String,
    held: Vec<npcs::Held>,
}

/// A figure, on its pedestal; its glTF until its scene is in place.
#[derive(Component)]
struct Figure {
    pedestal: usize,
    gltf: Option<Handle<Gltf>>,
    idle: String,
}

/// A figure's own copies of its meshes' materials with their colours and reflectances lit, which
/// its light changes alone, and whether it stands lit.
#[derive(Component, Default)]
struct Shades {
    materials: Vec<(Shaded, Color, f32)>,
    lit: Option<bool>,
}

/// A figure's own copy of a mesh's material: Bevy's own, or one over its item's effect of the
/// kinds 1 to 4, which the original draws on the figures as on a game's models (`0x6F1B30`,
/// `model_effects::ApartMaterial`; #1875).
enum Shaded {
    Plain(Handle<StandardMaterial>),
    Over(Handle<model_effects::ApartMaterial>),
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
    /// The erase button and the title of the Underworld campaign's level.
    Erase,
    Minimum,
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

/// The picture as the converter assembles it from its 12 tiles (`opensacred-convert ui`, #2634).
const PICTURE: &str = "ui/screens/choice.ktx2";

/// The picture (as the converter assembles it from `GUI_UW_CHAR_01` to `12`, #2634) and water,
/// and the panel's atlases, relative to the asset directory.
#[derive(Resource, Default)]
pub struct Images {
    background: menu::Background,
    bed: Option<String>,
    reflection: Option<String>,
    frames: Option<String>,
    fields: Option<String>,
}

impl Images {
    /// Finds the screen's textures among the converted ones.
    pub fn find(assets: &std::path::Path) -> Self {
        let names = [
            "ui_char_center",
            "ui_char_ref",
            "gui_main_12",
            "gui_main_13",
        ];
        let mut found = hud::find_textures(assets, &names);
        let fields = found.pop().flatten();
        let frames = found.pop().flatten();
        let reflection = found.pop().flatten();
        let bed = found.pop().flatten();
        Self {
            background: menu::Background::find(assets, PICTURE),
            bed,
            reflection,
            frames,
            fields,
        }
    }
}

/// The atlases of the interface that the panel's pieces are cut from.
#[derive(Clone, Copy)]
pub(crate) enum Sheet {
    /// `gui_main_12`: the frame, the rule, the buttons and the difficulty's bar.
    Frames,
    /// `gui_main_13`: the name's field.
    Fields,
}

/// The panel's atlases, loaded to be sampled texel by texel: the original draws its pieces a
/// texel to a pixel, and stretches a button's middle without blurring it.
pub(crate) struct Atlases {
    frames: Option<Handle<Image>>,
    fields: Option<Handle<Image>>,
}

impl Atlases {
    pub(crate) fn load(images: &Images, assets: &AssetServer) -> Self {
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
pub(crate) struct Piece {
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
/// A read-only field's left end (332), its right end's texels turned across, which the Underworld
/// campaign's name's field takes in place of the emblem (seen), as the saves' window's read-only
/// fields (`saves_window`).
const FIELD_PLAIN_LEFT: Piece = Piece::new(Sheet::Fields, [68, 118, 97, 143]).turned(true, false);
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
pub(crate) const SHADE: Color = Color::linear_rgba(0.0, 0.0, 0.0, 159.0 / 255.0);
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

/// The pedestal of the arrows' next choice from `chosen`, `step` figures on in the screen's
/// order.
fn step(chosen: Option<usize>, step: isize) -> usize {
    let count = ORDER.len();
    match chosen {
        Some(at) => {
            let signed = isize::try_from(at).unwrap_or(0) + step;
            usize::try_from(signed.rem_euclid(isize::try_from(count).unwrap_or(1))).unwrap_or(0)
        }
        None if step < 0 => count - 1,
        None => 0,
    }
}

/// Chooses the figure on `pedestal`, lit: a class's, its name the class's own; a saved hero of
/// the Underworld's level, or of any level for the import, its name its own and the difficulties
/// offered by those it won (`0x70CA30`). A free pedestal, or a hero below that level in the
/// Underworld campaign's choice, is not chosen; the export takes any pedestal.
fn choose(choice: &mut Choice, words: &Words, pedestal: usize) {
    let Some(stand) = choice.stand(pedestal).cloned() else {
        // The export takes a free pedestal too (seen).
        if choice.exporting {
            choice.chosen = Some(pedestal);
        }
        return;
    };
    match stand.saved {
        // The export's panel shows nothing of the hero.
        Some(_) if choice.exporting => {}
        None => {
            let class = stand.class;
            choice.name = words.get(texts(class).0).unwrap_or(class.name()).to_owned();
        }
        Some(saved) if choice.importing || saved.level >= UNDERWORLD_LEVEL => {
            choice.name = saved.name;
            choice.offered = offered_to(saved.highest);
            if !choice.offered.contains(&choice.difficulty) {
                choice.difficulty = Difficulty::default();
            }
        }
        Some(_) => return,
    }
    choice.chosen = Some(pedestal);
}

/// The class of a creature type, among the heroes'.
fn class_of(creature: u32) -> Option<Class> {
    ORDER.into_iter().find(|class| class.creature() == creature)
}

/// Reads the figures from the client's table and the game's data, once: the classes', each on
/// the pedestal of its entry, and the saved heroes' of `heroes.ron`, each on the pedestal of its
/// slot, in its summary's items (`0x70C7E0`, `0x6F4E60`); none where the table was not
/// converted.
fn read_figures(
    data: Option<Res<Data>>,
    settings: Option<Res<crate::settings::SettingsFile>>,
    mut figures: ResMut<Figures>,
) {
    let Some(data) = data else {
        return;
    };
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
            figures.0 = Some(Lineup::default());
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
    let classes = table
        .iter()
        .enumerate()
        .filter_map(|(pedestal, figure)| {
            let class = class_of(figure.class)?;
            let stand = Stand { class, saved: None };
            Some(look(
                shared,
                &files,
                (pedestal, figure),
                stand,
                &slots(figure, class),
            ))
        })
        .collect();
    // The player's exported heroes, each on its slot's pedestal over the installation's there
    // (#2552; docs/decisions/interface.md, "The installation's Underworld heroes").
    let exported = crate::exports::directory_of(settings.as_deref())
        .map(|dir| crate::exports::list(&dir))
        .unwrap_or_default();
    let taken: Vec<u8> = exported.iter().map(|(slot, _)| *slot).collect();
    let installed = saved_heroes(assets)
        .into_iter()
        .filter(|hero| !taken.contains(&hero.slot))
        .filter_map(|hero| {
            let class = class_of(hero.hero.class)?;
            let saved = Saved {
                name: hero.name,
                level: hero.hero.level,
                highest: highest_won(&hero.won),
                exported: None,
            };
            Some((hero.slot, class, saved, hero.equipment))
        });
    let own = exported.into_iter().map(|(slot, summary)| {
        let won: Vec<bool> = summary.wins.iter().map(|&count| count > 0).collect();
        let saved = Saved {
            name: summary.name,
            level: summary.level,
            highest: highest_won(&won),
            exported: Some(slot),
        };
        (slot, summary.class, saved, summary.equipment)
    });
    let saved = installed
        .chain(own)
        .filter_map(|(slot, class, saved, equipment)| {
            let pedestal = usize::from(slot);
            let figure = table.get(pedestal)?;
            let stand = Stand {
                class,
                saved: Some(saved),
            };
            let slots = dressed(equipment, class);
            Some(look(shared, &files, (pedestal, figure), stand, &slots))
        })
        .collect();
    let windows = table.iter().map(window_of).collect();
    figures.0 = Some(Lineup {
        classes,
        saved,
        windows,
    });
}

/// The saved heroes of the Underworld campaign's choice (`heroes.ron`'s `saved`, #1676), none
/// where the index lacks them.
fn saved_heroes(assets: &std::path::Path) -> Vec<opensacred_data::heroes::SavedHero> {
    let path = assets.join("heroes").join(opensacred_data::heroes::INDEX);
    match files::read_to_string(&path)
        .map(|text| ron::from_str::<opensacred_data::heroes::Heroes>(&text))
    {
        Ok(Ok(index)) => index.saved,
        Ok(Err(e)) => {
            warn!("choice: no saved heroes: parsing {}: {e}", path.display());
            Vec::new()
        }
        Err(e) => {
            warn!("choice: no saved heroes: reading {}: {e}", path.display());
            Vec::new()
        }
    }
}

/// A figure of `stand` on `pedestal`, at the place of the table's `figure`, wearing `slots`'
/// items: its model, the items' models, and its idle clip by what it holds.
fn look(
    shared: &crate::sectors::Shared,
    files: &(
        std::collections::HashMap<String, String>,
        std::collections::HashMap<u32, String>,
    ),
    (pedestal, figure): (usize, &Showcase),
    stand: Stand,
    slots: &[(u8, u32)],
) -> Look {
    let class = stand.class;
    let assets = &shared.assets;
    let (held_class, held) = match npcs::figure_gear(&shared.tables, (&files.0, &files.1), slots) {
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
            // The Vampiress and the Dark Elf stand in their fight stance, the rest as they stand
            // by what they hold (`0x6F4C58`).
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
    Look {
        stand,
        pedestal,
        window: window_of(figure),
        place: place_of(figure),
        model,
        idle,
        held,
    }
}

/// The window of a figure of the table in the picture, in which the cursor lights it.
fn window_of(figure: &Showcase) -> Rect {
    let [left, top, width, height] = figure.window;
    Rect::new(left, top, left + width, top + height)
}

/// The items of a figure of the table in their slots, and its class's default in its slot where
/// that is empty (`0x6F4E60`).
fn slots(figure: &Showcase, class: Class) -> Vec<(u8, u32)> {
    let slots: Vec<(u8, u32)> = SLOTS
        .iter()
        .zip(figure.items)
        .filter(|&(_, item)| item != 0)
        .filter_map(|(&slot, item)| Some((slot?, item)))
        .collect();
    dressed(slots, class)
}

/// The items of `slots`, and the class's default in its slot where that is empty (`0x6F4E60`).
fn dressed(mut slots: Vec<(u8, u32)>, class: Class) -> Vec<(u8, u32)> {
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

/// The figures' stage of a choice's screen of `scope`, `height` pixels high: their camera, its
/// texture of its frustum's shape, their light and the figures of `looks`; answers the layer that
/// lays the texture on the interface.
fn stage(
    commands: &mut Commands,
    (scope, looks, height): (Scope, &[Look], f32),
    (targets, layers, assets): (&mut Assets<Image>, &mut Assets<Layer>, &AssetServer),
) -> Handle<Layer> {
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
    scope.bind(&mut commands.spawn((
        // Its own picture's, which the world's camera's following and zoom leave alone over a
        // game (the export, #2554).
        crate::iso::Apart,
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
        // Lit by an ambient and a directional light alone: one cluster for the lights (#2664).
        bevy::light::cluster::ClusterConfig::Single,
        AmbientLight {
            color: Color::WHITE,
            brightness: AMBIENT,
            affects_lightmapped_meshes: true,
        },
        RenderLayers::layer(FIGURES),
    )));
    scope.bind(&mut commands.spawn((
        DirectionalLight {
            illuminance: LIGHT,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-1.0, 1.2, 1.5).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(FIGURES),
    )));
    for look in looks {
        let Some(model) = &look.model else {
            continue;
        };
        scope.bind(&mut commands.spawn((
            Figure {
                pedestal: look.pedestal,
                gltf: Some(assets.load(model.clone())),
                idle: look.idle.clone(),
            },
            Shades::default(),
            npcs::equipment(assets, &look.held),
            look.place,
            Visibility::default(),
            RenderLayers::layer(FIGURES),
            // Its scene's and its items' entities in the figures' layer from their first frame,
            // so that the world's camera never draws one over the screen while it is hidden
            // ([`unveil`], #2490).
            bevy::app::Propagate(RenderLayers::layer(FIGURES)),
        )));
    }
    layers.add(Layer { figures: target })
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
    *choice = Choice {
        scope: Some(Scope::Menu(Screen::Heroes)),
        ..default()
    };
    choice.stands = ORDER.map(|class| Some(Stand { class, saved: None }));
    let named = request.0.difficulty;
    choice.difficulty = named.unwrap_or_default();
    choice.offered = offered(named);
    let height = menu::picture_height(window.map(|window| window.height()));
    let water = ripples.spawn(
        &mut commands,
        (&mut targets, &assets),
        (Screen::Heroes, &pond(&images, true)),
    );
    let looks = figures
        .0
        .as_ref()
        .map_or(&[][..], |lineup| lineup.classes.as_slice());
    let layer = stage(
        &mut commands,
        (Scope::Menu(Screen::Heroes), looks, height),
        (&mut targets, &mut layers, &assets),
    );
    let font: Handle<Font> = assets.load(FONT);
    picture(
        &mut commands,
        (Scope::Menu(Screen::Heroes), &images, &assets),
        water,
        |picture| {
            picture.spawn((
                MaterialNode(layer),
                place(Rect::from_corners(Vec2::ZERO, menu::SCREEN)),
            ));
            for look in looks {
                spotted(picture, look.pedestal, look.window);
            }
            let atlases = Atlases::load(&images, &assets);
            let importable = importable(&figures);
            panel(
                picture,
                &atlases,
                &words,
                &font,
                Kind::Classes { importable },
            );
        },
    );
}

/// Whether a saved hero stands on a pedestal, the installation's or one the player exported,
/// which the Ancaria campaign's Import takes (`0x70CEF6`, the count of `0x70C4A0`).
fn importable(figures: &Figures) -> bool {
    figures
        .0
        .as_ref()
        .is_some_and(|lineup| !lineup.saved.is_empty())
}

/// The choice of a saved hero: the Underworld campaign's (#1129, #1676) or the Ancaria
/// campaign's import (the mode 7 in either, #2552): the same picture, the saved heroes standing
/// on their pedestals, each labelled with its name and level, and the free pedestals labelled
/// free; the panel's name, the difficulty once a hero is chosen, the erase button while a hero
/// the player exported is chosen, Back and Start Game, greyed until a hero is chosen. The
/// Underworld campaign's has no Import (`0x70D25C`) and titles the level a hero needs while the
/// cursor is on one below it (`0x70F450`); the import has in Import's place the button back to
/// the classes. The export over a game (the mode 5, #2554) titles itself local and has Back and
/// Export alone, greyed until a pedestal is chosen, any pedestal. No water of theirs has a
/// reflection, which is the classes' alone (`0x70E8A0`: the modes 0 to 2).
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources one by one"
)]
fn saved_screen(
    mut commands: Commands,
    (images, ui_images): (Res<Images>, Res<UiImages>),
    (words, screen): (Res<Words>, Option<Res<State<Screen>>>),
    exporting: Option<Res<State<Exporting>>>,
    figures: Res<Figures>,
    assets: Res<AssetServer>,
    window: Option<Single<&Window, With<PrimaryWindow>>>,
    (mut targets, mut layers): (ResMut<Assets<Image>>, ResMut<Assets<Layer>>),
    mut choice: ResMut<Choice>,
    mut ripples: Ripples,
) {
    let scope = if exporting.is_some_and(|exporting| *exporting.get() == Exporting::Shown) {
        Scope::Export
    } else if let Some(screen) = screen {
        Scope::Menu(*screen.get())
    } else {
        return;
    };
    let kind = match scope {
        Scope::Export => Kind::Export,
        Scope::Menu(Screen::Import) => Kind::Import,
        Scope::Menu(_) => Kind::Underworld,
    };
    *choice = Choice {
        importing: kind == Kind::Import,
        exporting: kind == Kind::Export,
        scope: Some(scope),
        ..default()
    };
    let (looks, windows) = figures.0.as_ref().map_or((&[][..], &[][..]), |lineup| {
        (lineup.saved.as_slice(), lineup.windows.as_slice())
    });
    for look in looks {
        if let Some(stand) = choice.stands.get_mut(look.pedestal) {
            *stand = Some(look.stand.clone());
        }
    }
    let height = menu::picture_height(window.map(|window| window.height()));
    let pond = pond(&images, false);
    let water = match scope {
        Scope::Menu(screen) => {
            ripples.spawn(&mut commands, (&mut targets, &assets), (screen, &pond))
        }
        Scope::Export => ripples.spawn(
            &mut commands,
            (&mut targets, &assets),
            (Exporting::Shown, &pond),
        ),
    };
    let layer = stage(
        &mut commands,
        (scope, looks, height),
        (&mut targets, &mut layers, &assets),
    );
    let font: Handle<Font> = assets.load(FONT);
    let label_font: Handle<Font> = assets.load(MENU_FONT);
    let atlases = Atlases::load(&images, &assets);
    picture(&mut commands, (scope, &images, &assets), water, |picture| {
        picture.spawn((
            MaterialNode(layer),
            place(Rect::from_corners(Vec2::ZERO, menu::SCREEN)),
        ));
        for (pedestal, &window) in windows.iter().enumerate() {
            spotted(picture, pedestal, window);
        }
        for (pedestal, &at) in PEDESTALS.iter().enumerate() {
            match looks.iter().find(|look| look.pedestal == pedestal) {
                Some(look) => {
                    hero_label(picture, (look, &words, &label_font), (&ui_images, &assets))
                }
                None => free_label(picture, pedestal, at, (&words, &font)),
            }
        }
        panel(picture, &atlases, &words, &font, kind);
        match kind {
            Kind::Underworld => {
                erase_button(picture, (&ui_images, &assets));
                let fallback = format!("A hero of level {UNDERWORLD_LEVEL} or more");
                let text = words.get(MINIMUM).unwrap_or(&fallback);
                let minimum = title(picture, (text, &font), MINIMUM_COLOUR);
                picture
                    .commands()
                    .entity(minimum)
                    .insert((Part::Minimum, Visibility::Hidden));
            }
            Kind::Import => erase_button(picture, (&ui_images, &assets)),
            Kind::Export => {
                let text = words.get(EXPORT_TITLE).unwrap_or("Export (local)");
                title(picture, (text, &font), EXPORT_COLOUR);
            }
            Kind::Classes { .. } => {}
        }
    });
}

/// The erase button at the name's field's right end, shown while a hero the player exported is
/// chosen (`0x70CA30`; [`show`]); a click asks whether to erase it ([`ask_erase`]).
fn erase_button(picture: &mut ChildSpawnerCommands, (images, assets): (&UiImages, &AssetServer)) {
    let (image, node) = bar::piece(images, assets, ERASE, ERASE_AT, Vec2::ZERO);
    picture
        .spawn((Part::Erase, Visibility::Hidden, image, node))
        .observe(|click: On<PointerClick>, mut commands: Commands| {
            if click.button == PointerButton::Primary {
                commands.run_system_cached(ask_erase);
            }
        });
}

/// The erase button's tooltip (#2556): `UI_CHAR_ERASE_TOOLTIP` once the cursor has rested on the
/// shown button for the controls' rest (`tooltip::TIP_AFTER`, `0x7327B0`), centred 20 over the
/// middle of its top as the bar's controls' are (**hypothesis**, docs/re/interface.md), gone as
/// the cursor leaves it; the cursor's place is tested, as the tooltip over the button
/// would take the button's hover.
fn erase_tip(
    mut commands: Commands,
    (time, choice): (Res<Time<Real>>, Res<Choice>),
    (pointer, ui_scale, windows): (
        Res<crate::hud::Pointer>,
        Res<UiScale>,
        Query<&bevy::window::Window>,
    ),
    (assets, words): (Res<AssetServer>, Res<Words>),
    tips: Query<Entity, With<crate::tooltip::TipRoot>>,
    mut rest: Local<crate::tooltip::Rest>,
) {
    let button = Rect::from_corners(ERASE_AT, ERASE_AT + Vec2::splat(2.0 * ERASE_HALF));
    let on = chosen_export(&choice).is_some()
        && crate::inventory::screen_point(&pointer, &ui_scale, &windows)
            .is_some_and(|at| button.contains(at));
    let (gone, due) = rest.step(on.then_some(0), time.elapsed_secs());
    if gone {
        for tip in &tips {
            commands.entity(tip).despawn();
        }
    }
    if due.is_some() {
        let text = words.get(ERASE_TIP).unwrap_or("Delete");
        crate::tooltip::spawn_tip(
            &mut commands,
            &assets,
            text,
            ERASE_AT + Vec2::new(ERASE_HALF, -TIP_ABOVE),
        );
    }
}

/// The tooltips of a choice's screen gone with it.
fn drop_tips(mut commands: Commands, tips: Query<Entity, With<crate::tooltip::TipRoot>>) {
    for tip in &tips {
        commands.entity(tip).despawn();
    }
}

/// A title across the screen ([`MINIMUM_TITLE`], `0x70F450`) of `text` in `colour`.
fn title(
    picture: &mut ChildSpawnerCommands,
    (text, font): (&str, &Handle<Font>),
    colour: u32,
) -> Entity {
    picture
        .spawn((
            Pickable::IGNORE,
            place(MINIMUM_TITLE),
            Text::new(text),
            TextFont {
                font: font.clone().into(),
                font_size: FontSize::Px(MINIMUM_SIZE),
                ..default()
            },
            TextLayout::justify(Justify::Center),
            TextColor(crate::tooltip::argb(colour)),
        ))
        .id()
}

/// A question of the choice's in the busy dialog, OK and Cancel, and its answers: erase the
/// exported hero of a slot, export the hero over the slot's, or nothing.
#[derive(Component)]
struct Question;
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum Answer {
    Erase(u8),
    Overwrite(u8),
    Cancel,
}

/// Asks `line` in the busy dialog over the screen of `scope`, OK answering `yes`.
fn ask(
    commands: &mut Commands,
    (images, assets, texts): (&Images, &AssetServer, Option<&crate::lines::Texts>),
    scope: Scope,
    (line, yes): ((&str, &str), Answer),
) {
    let buttons = [
        (yes, crate::busy_dialog::OK_BESIDE, crate::busy_dialog::OK),
        (
            Answer::Cancel,
            crate::busy_dialog::CANCEL,
            ("1025", "Cancel"),
        ),
    ];
    let spawned = match scope {
        Scope::Menu(screen) => crate::busy_dialog::spawn(
            commands,
            (images, assets, texts),
            ((Question, DespawnOnExit(screen)), line),
            &buttons,
        ),
        Scope::Export => crate::busy_dialog::spawn(
            commands,
            (images, assets, texts),
            ((Question, DespawnOnExit(Exporting::Shown)), line),
            &buttons,
        ),
    };
    for (button, answer) in spawned {
        commands
            .entity(button)
            .observe(move |click: On<PointerClick>, mut commands: Commands| {
                if click.button == PointerButton::Primary {
                    commands.run_system_cached_with(answered, answer);
                }
            });
    }
}

/// Asks whether to erase the chosen hero, one the player exported (`0x70EE60`:
/// `UI_CHAR_ERASE_CONFIRM` in the busy dialog, OK and Cancel).
fn ask_erase(
    mut commands: Commands,
    choice: Res<Choice>,
    (images, assets, texts): (
        Res<Images>,
        Res<AssetServer>,
        Option<Res<crate::lines::Texts>>,
    ),
    questions: Query<(), With<Question>>,
) {
    let (Some(slot), Some(scope)) = (chosen_export(&choice), choice.scope) else {
        return;
    };
    if !questions.is_empty() {
        return;
    }
    ask(
        &mut commands,
        (&images, &assets, texts.as_deref()),
        scope,
        ((ERASE_CONFIRM, "Delete the hero?"), Answer::Erase(slot)),
    );
}

/// Export: the hero written to the chosen pedestal's slot (`0x70EC50`), where a hero stands there
/// after `UI_MSG_OVERWRITE_CHAR` (the action 9, `0x70D71E`).
fn export_chosen(
    mut commands: Commands,
    choice: Res<Choice>,
    (images, assets, texts): (
        Res<Images>,
        Res<AssetServer>,
        Option<Res<crate::lines::Texts>>,
    ),
    questions: Query<(), With<Question>>,
) {
    let Some(pedestal) = choice.chosen.filter(|_| choice.exporting) else {
        return;
    };
    let Ok(slot) = u8::try_from(pedestal) else {
        return;
    };
    if !questions.is_empty() {
        return;
    }
    if choice.stand(pedestal).is_some() {
        ask(
            &mut commands,
            (&images, &assets, texts.as_deref()),
            Scope::Export,
            ((OVERWRITE, "Overwrite the hero?"), Answer::Overwrite(slot)),
        );
    } else {
        commands.queue(move |world: &mut World| write_export(world, slot));
    }
}

/// Writes the game's hero to the slot `slot` (the action 6, `0x70E502`) and shows the export
/// again, the new hero on its pedestal.
fn write_export(world: &mut World, slot: u8) {
    match crate::exports::export(world, slot) {
        Ok(path) => info!("choice: the hero exported to {}", path.display()),
        Err(e) => warn!("choice: exporting the hero: {e:#}"),
    }
    if let Some(mut next) = world.get_resource_mut::<NextState<Exporting>>() {
        next.set(Exporting::Shown);
    }
}

/// The slot of the chosen hero where the player exported it.
fn chosen_export(choice: &Choice) -> Option<u8> {
    choice
        .chosen
        .and_then(|pedestal| choice.stand(pedestal))
        .and_then(|stand| stand.saved.as_ref())
        .and_then(|saved| saved.exported)
}

/// A question answered: the erase's OK erases the hero's file (`0x720FDC`, `0x7F0CB0`) and
/// fills the pedestals again, the screen made anew; the overwrite's exports the hero over the
/// slot's (`0x70EE60`'s answer 2); either answer closes the question.
fn answered(
    In(answer): In<Answer>,
    mut commands: Commands,
    questions: Query<Entity, With<Question>>,
    (choice, settings): (Res<Choice>, Option<Res<crate::settings::SettingsFile>>),
    screen: Option<ResMut<NextState<Screen>>>,
    export: Option<ResMut<NextState<Exporting>>>,
) {
    for question in &questions {
        commands.entity(question).despawn();
    }
    match answer {
        Answer::Cancel => {}
        Answer::Overwrite(slot) => {
            commands.queue(move |world: &mut World| write_export(world, slot));
        }
        Answer::Erase(slot) => {
            let Some(dir) = crate::exports::directory_of(settings.as_deref()) else {
                return;
            };
            match crate::exports::erase(&dir, slot) {
                Ok(()) => info!("choice: the exported hero of the slot {slot} erased"),
                Err(e) => warn!("choice: {e:#}"),
            }
            match (choice.scope, screen, export) {
                (Some(Scope::Menu(at)), Some(mut screen), _) => screen.set(at),
                (Some(Scope::Export), _, Some(mut export)) => export.set(Exporting::Shown),
                _ => {}
            }
        }
    }
}

/// A free pedestal's label, centred on `at`: `UI_SAVE_EMPTY`'s word, white, yellow while the
/// cursor is in its window or the pedestal is chosen (`0x6F15A0`).
fn free_label(
    picture: &mut ChildSpawnerCommands,
    pedestal: usize,
    at: Vec2,
    (words, font): (&Words, &Handle<Font>),
) {
    picture.spawn((
        Pickable::IGNORE,
        Node {
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..place(about(at, PEDESTAL_LABEL))
        },
        // The word passes a click to its pedestal's window under it, as the original's (seen).
        children![(
            Label::Free(pedestal),
            Pickable::IGNORE,
            Text::new(words.get(EMPTY).unwrap_or("<empty>")),
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

/// A saved hero's label at the foot of its figure's window (`0x6F15A0`): its box, its name, and
/// its level with the medal of the highest difficulty it won, each a line unbroken, wider than a
/// narrow window's box where it is longer (#2552).
fn hero_label(
    picture: &mut ChildSpawnerCommands,
    (look, words, font): (&Look, &Words, &Handle<Font>),
    (images, assets): (&UiImages, &AssetServer),
) {
    let Some(saved) = &look.stand.saved else {
        return;
    };
    let colour = crate::tooltip::argb(look.stand.class.colour());
    let foot = look.window.max.y;
    let top = foot - 2.0 * LABEL_LINE;
    let lettering = (
        TextFont {
            font: font.clone().into(),
            font_size: FontSize::Px(LABEL_SIZE),
            ..default()
        },
        LineHeight::Px(LABEL_LINE),
        TextColor(colour),
    );
    let level = format!("{}: {} ", words.get(LEVEL).unwrap_or("Level"), saved.level);
    // The medals end with niobium's, the highest won's past it none (`0x6F15A0`).
    let medal = u32::try_from(saved.highest)
        .ok()
        .filter(|&highest| highest <= 4)
        .map(|highest| bar::piece(images, assets, MEDALS + highest, Vec2::ZERO, Vec2::ZERO).0);
    picture
        .spawn((
            Label::Hero(look.pedestal),
            Pickable::IGNORE,
            place(Rect::new(
                look.window.min.x,
                top,
                look.window.max.x,
                foot + LABEL_FOOT,
            )),
            BackgroundColor(SHADE),
        ))
        .with_children(|label| {
            label.spawn((
                Pickable::IGNORE,
                Node {
                    justify_content: JustifyContent::Center,
                    ..place(Rect::new(0.0, 0.0, look.window.width(), LABEL_LINE))
                },
                children![(
                    Pickable::IGNORE,
                    Text::new(saved.name.clone()),
                    lettering.clone(),
                    TextLayout::new(Justify::Left, LineBreak::NoWrap),
                )],
            ));
            label
                .spawn((
                    Pickable::IGNORE,
                    Node {
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..place(Rect::new(
                            0.0,
                            LABEL_LINE,
                            look.window.width(),
                            2.0 * LABEL_LINE,
                        ))
                    },
                ))
                .with_children(|line| {
                    line.spawn((
                        Pickable::IGNORE,
                        Text::new(level),
                        lettering,
                        TextLayout::new(Justify::Left, LineBreak::NoWrap),
                    ));
                    if let Some(medal) = medal {
                        line.spawn((
                            Pickable::IGNORE,
                            medal,
                            Node {
                                width: Val::Px(MEDAL),
                                height: Val::Px(MEDAL),
                                ..default()
                            },
                        ));
                    }
                });
        });
}

/// A pedestal's label, of the Underworld campaign's choice: a free pedestal's word, or a saved
/// hero's box.
#[derive(Component, Clone, Copy)]
enum Label {
    Free(usize),
    Hero(usize),
}

/// Colours the Underworld campaign's labels as the choice stands: a free pedestal's word yellow
/// while the cursor is in its window or it is chosen, a saved hero's box blue while it is chosen
/// (`0x6F15A0`).
fn colour_labels(
    choice: Res<Choice>,
    mut labels: Query<(&Label, Option<&mut TextColor>, &mut BackgroundColor)>,
) {
    if !choice.is_changed() {
        return;
    }
    for (&label, text, mut ground) in &mut labels {
        match label {
            Label::Free(pedestal) => {
                let lit = choice.chosen == Some(pedestal) || choice.hovered == Some(pedestal);
                if let Some(mut text) = text {
                    text.0 = if lit { LIT } else { TEXT };
                }
            }
            Label::Hero(pedestal) => {
                ground.0 = if choice.chosen == Some(pedestal) {
                    CHOSEN_BOX
                } else {
                    SHADE
                };
            }
        }
    }
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
    (scope, images, assets): (Scope, &Images, &AssetServer),
    water: Handle<Image>,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) {
    let (layer, veil) = scope.layers();
    // Black over it until all of it has loaded ([`unveil`], #2490): under the veil its pictures,
    // its figures' layer and their materials are made as when shown, so that it shows whole.
    scope.bind(&mut commands.spawn((
        Veil,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::BLACK),
        GlobalZIndex(veil),
    )));
    let mut root = commands.spawn((
        Veiled::default(),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::BLACK),
    ));
    scope.bind(&mut root);
    if let Some(layer) = layer {
        root.insert(GlobalZIndex(layer));
    }
    root.with_children(|root| {
        root.spawn(Node {
            width: Val::Px(menu::SCREEN.x),
            height: Val::Px(menu::SCREEN.y),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|picture| {
            menu::water(picture, water);
            images.background.spawn(picture, assets, ());
            content(picture);
        });
    });
}

/// The node of the picture's rectangle `rect`.
pub(crate) fn place(rect: Rect) -> Node {
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

/// A pedestal's window, the cursor over it lighting the figure on `pedestal` and a click
/// choosing it.
fn spotted(picture: &mut ChildSpawnerCommands, pedestal: usize, window: Rect) {
    picture
        .spawn((place(window),))
        .observe(move |_: On<PointerOver>, mut choice: ResMut<Choice>| {
            choice.hovered = Some(pedestal);
        })
        .observe(move |_: On<PointerOut>, mut choice: ResMut<Choice>| {
            if choice.hovered == Some(pedestal) {
                choice.hovered = None;
            }
        })
        .observe(
            move |click: On<PointerClick>, mut choice: ResMut<Choice>, words: Res<Words>| {
                if click.button == PointerButton::Primary {
                    choose(&mut choice, &words, pedestal);
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
    button_pieces(parent, atlases, rect);
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

/// The name's field: its middle stretched over `NAME_FIELD`, its emblem at the left, or a plain
/// end where not `editable`, and its end at the right as wide as its middle, as the original's
/// edit control draws them (`0x7382F0`).
fn field(parent: &mut ChildSpawnerCommands, atlases: &Atlases, editable: bool) {
    let left = if editable {
        FIELD_LEFT
    } else {
        FIELD_PLAIN_LEFT
    };
    row(
        parent,
        atlases,
        [left, FIELD_MIDDLE, FIELD_RIGHT],
        NAME_FIELD,
        [left.size().x, FIELD_MIDDLE.size().x],
    );
}

/// The name's field and the name in it.
fn name(parent: &mut ChildSpawnerCommands, atlases: &Atlases, font: &Handle<Font>, editable: bool) {
    field(parent, atlases, editable);
    parent.spawn((
        Part::Name,
        Pickable::IGNORE,
        line(NAME_AT.x, NAME_FIELD.max.x, NAME_AT.y),
        Text::new(""),
        lettering(font),
        TextColor(TEXT),
    ));
}

/// Which choice a panel is under: the Ancaria campaign's classes, whose Import is enabled where
/// `importable`, the Underworld campaign's saved heroes, the Ancaria campaign's import of one, or
/// the export over a game.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Classes { importable: bool },
    Underworld,
    Import,
    Export,
}

/// The panel under the figures, of the choice `kind`.
fn panel(
    picture: &mut ChildSpawnerCommands,
    atlases: &Atlases,
    words: &Words,
    font: &Handle<Font>,
    kind: Kind,
) {
    frame(picture, atlases);
    controls(picture, atlases, (words, font), kind);
    list(picture, words, font);
}

/// The name of `difficulty` in the game's texts.
fn difficulty_name(words: &Words, difficulty: Difficulty) -> &str {
    let index = difficulty.index();
    words
        .get(DIFFICULTIES[index])
        .unwrap_or(crate::game::DIFFICULTIES[index])
}

/// The bottom of the difficulties' list's dark box of `rows` rows.
fn list_bottom(rows: usize) -> f32 {
    LIST_TOP + LIST_BOX + LIST_BOX_PITCH * f32::from(u8::try_from(rows).unwrap_or(u8::MAX))
}

/// A row of the difficulties' list, of the difficulty of its index, shown while it is offered;
/// and the list's dark box, as tall as the rows offered.
#[derive(Component)]
enum Listed {
    Row(usize),
    Box,
}

/// The difficulties' list, over the panel: a catch of the clicks elsewhere, which closes it, its
/// dark box, and each difficulty, yellow under the cursor and chosen by a click; `show` shows the
/// rows of those offered.
fn list(picture: &mut ChildSpawnerCommands, words: &Words, font: &Handle<Font>) {
    let bottom = list_bottom(Difficulty::ALL.len());
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
                Listed::Box,
                Pickable::IGNORE,
                place(Rect::new(left, LIST_TOP, right, bottom)),
                BackgroundColor(LIST_SHADE),
            ));
            for (row, difficulty) in Difficulty::ALL.into_iter().enumerate() {
                let foot = LIST_FIRST + LIST_PITCH * f32::from(u8::try_from(row).unwrap_or(0));
                list.spawn((
                    Listed::Row(row),
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

/// The panel's frame, as the original draws it about the box `FRAMED`: its dark fill, the frame
/// ([`frame_about`]), and the rule over the buttons, its middles up to its right end, drawn last
/// over the last of them.
fn frame(picture: &mut ChildSpawnerCommands, atlases: &Atlases) {
    picture.spawn((Pickable::IGNORE, place(FILL), BackgroundColor(SHADE)));
    frame_about(picture, atlases, FRAMED);
    put(picture, atlases, RULE_LEFT, RULE_AT);
    let end = RULE_AT.x + RULE_WIDTH - RULE_RIGHT.size().x;
    let mut x = RULE_AT.x + RULE_LEFT.size().x;
    while x < end {
        put(picture, atlases, RULE_MIDDLE, Vec2::new(x, RULE_AT.y));
        x += RULE_MIDDLE.size().x;
    }
    put(picture, atlases, RULE_RIGHT, Vec2::new(end, RULE_AT.y));
}

/// The interface's standard frame (`UI_STATIC_FRAME`), as the original draws it about the box
/// `framed`, here and over a game (the Esc menu, `esc_menu`): the crest's halves at the middle of
/// its top and its foot, its corners, its edges along the top and the foot, as many whole pieces
/// as the box is wide, and its sides down to the box's foot.
pub(crate) fn frame_about(picture: &mut ChildSpawnerCommands, atlases: &Atlases, framed: Rect) {
    let Rect { min, max } = framed;
    let top = min.y - CORNERS[0].size().y;
    let middle = framed.center().x;
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
}

/// A button's pieces about its middle `rect` (`BUTTON_LEFT`, `BUTTON_MIDDLE`, `BUTTON_RIGHT`), as
/// the choice's and the Esc menu's dialog's buttons are drawn: the button's whole rectangle, its
/// ends with it.
pub(crate) fn button_pieces(
    parent: &mut ChildSpawnerCommands,
    atlases: &Atlases,
    rect: Rect,
) -> Rect {
    let ends = [BUTTON_LEFT.size().x, BUTTON_RIGHT.size().x];
    row(
        parent,
        atlases,
        [BUTTON_LEFT, BUTTON_MIDDLE, BUTTON_RIGHT],
        rect,
        ends,
    );
    Rect::new(
        rect.min.x - ends[0],
        rect.min.y,
        rect.max.x + ends[1],
        rect.max.y,
    )
}

/// The panel's parts: the name and the difficulty, the Ancaria campaign's Import (where
/// `ancaria`), which a description hides, the description, Back and Start Game. The Ancaria
/// campaign's name shows with the difficulty once a class is chosen, in a field that takes the
/// keys; the Underworld campaign's always, in a plain field, which its hero's name fills as it is
/// chosen (seen).
fn controls(
    picture: &mut ChildSpawnerCommands,
    atlases: &Atlases,
    (words, font): (&Words, &Handle<Font>),
    kind: Kind,
) {
    if kind == Kind::Export {
        export_controls(picture, atlases, (words, font));
        return;
    }
    let ancaria = matches!(kind, Kind::Classes { .. });
    if !ancaria {
        name(picture, atlases, font, false);
    }
    picture
        .spawn((
            Part::Controls,
            Pickable::IGNORE,
            place(Rect::from_corners(Vec2::ZERO, menu::SCREEN)),
        ))
        .with_children(|controls| {
            if ancaria {
                name(controls, atlases, font, true);
            }
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
    // Import, greyed where no hero is saved (`0x70CF12`), and in the import's place the button
    // back to the classes (`0x70D25C`).
    let import = match kind {
        Kind::Classes { importable } => Some((
            words.get(IMPORT).unwrap_or("Import"),
            !importable,
            (|commands: &mut Commands| commands.run_system_cached(import)) as fn(&mut Commands),
        )),
        Kind::Import => Some((
            words.get(NEW_HERO).unwrap_or("New Hero"),
            false,
            (|commands: &mut Commands| commands.run_system_cached(classes)) as fn(&mut Commands),
        )),
        Kind::Underworld | Kind::Export => None,
    };
    if let Some((word, greyed, act)) = import {
        picture
            .spawn((
                Part::Import,
                Pickable::IGNORE,
                place(Rect::from_corners(Vec2::ZERO, menu::SCREEN)),
            ))
            .with_children(|import| {
                button(import, atlases, (word, font, greyed), IMPORT_BUTTON, act);
            });
    }
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

/// The export's panel (the mode 5, seen): Back, and Export on Start Game's button, greyed until
/// a pedestal is chosen ([`show`]).
fn export_controls(
    picture: &mut ChildSpawnerCommands,
    atlases: &Atlases,
    (words, font): (&Words, &Handle<Font>),
) {
    button(
        picture,
        atlases,
        (words.get(BACK).unwrap_or("Back"), font, false),
        BACK_BUTTON,
        |commands| commands.run_system_cached(back),
    );
    let export = button(
        picture,
        atlases,
        (words.get(EXPORT).unwrap_or("Export"), font, true),
        START_BUTTON,
        |commands| commands.run_system_cached(export_chosen),
    );
    picture.commands().entity(export).insert(Part::Start);
}

/// Back to the main menu, from the import back to the classes (seen), from the export back to
/// the menu over the game (seen).
fn back(
    mut commands: Commands,
    choice: Res<Choice>,
    screen: Option<ResMut<NextState<Screen>>>,
    export: Option<ResMut<NextState<Exporting>>>,
) {
    match (choice.scope, screen, export) {
        (Some(Scope::Menu(Screen::Import)), Some(mut screen), _) => screen.set(Screen::Heroes),
        (Some(Scope::Menu(_)), Some(mut screen), _) => screen.set(Screen::Main),
        (Some(Scope::Export), _, Some(mut export)) => leave_export(&mut commands, &mut export),
        _ => {}
    }
}

/// The export left for the menu over the game, the game still paused.
fn leave_export(commands: &mut Commands, export: &mut NextState<Exporting>) {
    export.set(Exporting::Hidden);
    commands.run_system_cached(crate::esc_menu::reopen);
}

/// Esc in the export closes its question as Cancel does, else goes back to the menu over the
/// game, before the pause takes the key.
fn export_keys(
    mut commands: Commands,
    mut keys: crate::controls::KeysMut,
    questions: Query<Entity, With<Question>>,
    mut export: ResMut<NextState<Exporting>>,
) {
    if !keys.take(crate::controls::Action::Back) {
        return;
    }
    if questions.is_empty() {
        leave_export(&mut commands, &mut export);
    }
    for question in &questions {
        commands.entity(question).despawn();
    }
}

/// While the game's time stands under the export, the figures idle by the real time, as the
/// original's screen draws them over its paused game (seen): their clips sought on by its
/// steps.
fn idle_paused(
    (real, game): (Res<Time<Real>>, Res<Time<Virtual>>),
    (clips, graphs): (Res<Assets<AnimationClip>>, Res<Assets<AnimationGraph>>),
    figures: Query<Entity, With<Figure>>,
    children: Query<&Children>,
    mut players: Query<(&mut AnimationPlayer, &AnimationGraphHandle)>,
) {
    if !game.is_paused() {
        return;
    }
    let delta = real.delta_secs();
    for figure in &figures {
        for entity in children.iter_descendants(figure) {
            let Ok((mut player, graph)) = players.get_mut(entity) else {
                continue;
            };
            let Some(graph) = graphs.get(graph) else {
                continue;
            };
            for (&node, active) in player.playing_animations_mut() {
                let duration = match graph.get(node).map(|node| &node.node_type) {
                    Some(bevy::animation::graph::AnimationNodeType::Clip(clip)) => {
                        clips.get(clip).map_or(0.0, AnimationClip::duration)
                    }
                    _ => 0.0,
                };
                if duration > 0.0 {
                    let time = (active.seek_time() + delta * active.speed()).rem_euclid(duration);
                    active.seek_to(time);
                }
            }
        }
    }
}

/// Import: the choice of a saved hero for the Ancaria campaign, where one is saved (`0x70CEF6`).
fn import(figures: Res<Figures>, mut screen: ResMut<NextState<Screen>>) {
    if importable(&figures) {
        screen.set(Screen::Import);
    }
}

/// The import's button back to the classes.
fn classes(mut screen: ResMut<NextState<Screen>>) {
    screen.set(Screen::Heroes);
}

/// Starts a game of the hero chosen, of its name, if one is: a class's of the Ancaria campaign,
/// or a saved hero's of the Underworld campaign, in its class's tree of the add-on, whose start
/// script places it (`IMPORT_DO`, `0x70DC20`; docs/re/underworld.md), the saved hero's values,
/// skills and combat arts carried (`carry`, #2362).
fn start(
    choice: Res<Choice>,
    mut request: ResMut<Request>,
    mut state: ResMut<NextState<AppState>>,
) {
    let Some(stand) = choice.chosen.and_then(|pedestal| choice.stand(pedestal)) else {
        return;
    };
    let class = stand.class;
    // The import's game is the Ancaria campaign's, in the class's own tree.
    let tree = stand
        .saved
        .as_ref()
        .filter(|_| !choice.importing)
        .map(|_| class.underworld_tree());
    info!(
        "choice: a {} named {}, on {:?}{}",
        class.name(),
        choice.name,
        choice.difficulty,
        tree.as_ref()
            .map_or(String::new(), |tree| format!(", in the tree {tree}"))
    );
    request.0.class = Some(class);
    request.0.name = Some(choice.name.clone());
    request.0.difficulty = Some(choice.difficulty);
    request.0.tree = tree;
    // The installation's saved hero's slot, its pedestal's, whose values its game carries
    // (#2362), or the slot of a hero the player exported, whose whole character it takes
    // (#2552).
    let slot = choice
        .chosen
        .and_then(|pedestal| u8::try_from(pedestal).ok());
    request.0.saved = stand
        .saved
        .as_ref()
        .filter(|saved| saved.exported.is_none())
        .and(slot);
    request.0.exported = stand.saved.as_ref().and_then(|saved| saved.exported);
    state.set(AppState::Loading);
}

/// The arrows choose the next figure or the one before, a key types the chosen hero's name and
/// Backspace takes its last character back, and Enter starts the game. Held back with
/// [`crate::frame::Control`], by the open console among others, its keys read and dropped so that
/// none typed into the console reaches the name once it closes; the tools' keys type nothing, the
/// console's among them (#2666).
fn keys(
    mut commands: Commands,
    (mut input, keys, held): (
        MessageReader<KeyboardInput>,
        crate::controls::Keys,
        Option<Res<crate::frame::HeldBack>>,
    ),
    mut choice: ResMut<Choice>,
    words: Res<Words>,
) {
    if held.is_some_and(|held| held.held()) {
        input.clear();
        return;
    }
    let tools = |code| {
        keys.controls()
            .in_section(crate::controls::Section::Tools, code)
    };
    for input in input.read() {
        if !input.state.is_pressed() || tools(input.key_code) {
            continue;
        }
        match &input.logical_key {
            Key::ArrowRight | Key::ArrowDown => {
                let pedestal = step(choice.chosen, 1);
                choose(&mut choice, &words, pedestal);
            }
            Key::ArrowLeft | Key::ArrowUp => {
                let pedestal = step(choice.chosen, -1);
                choose(&mut choice, &words, pedestal);
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

/// The panel as the choice stands: Start Game greyed until a hero is chosen, a class's
/// description in place of the controls while the cursor is on its figure, and the rows of the
/// difficulties offered.
fn show(
    choice: Res<Choice>,
    words: Res<Words>,
    mut parts: Query<(&Part, &mut Visibility, Option<&mut Text>, Option<&mut Word>)>,
    mut listed: Query<(&Listed, &mut Visibility, &mut Node), Without<Part>>,
    mut commands: Commands,
) {
    if !choice.is_changed() {
        return;
    }
    // The chosen hero's own figure shows no description, as the original's, nor a saved hero's.
    let description = choice
        .class_at(
            choice
                .hovered
                .filter(|&pedestal| choice.chosen != Some(pedestal)),
        )
        .map(|class| texts(class).1);
    for (listed, mut visibility, mut node) in &mut listed {
        match *listed {
            Listed::Row(row) => {
                *visibility = if row < choice.offered.len() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
            Listed::Box => {
                node.height = Val::Px(list_bottom(choice.offered.len()) - LIST_TOP);
            }
        }
    }
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
            Part::Erase => {
                *visibility = if chosen_export(&choice).is_some() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
            Part::Minimum => {
                let below = choice
                    .hovered
                    .and_then(|pedestal| choice.stand(pedestal))
                    .and_then(|stand| stand.saved.as_ref())
                    .is_some_and(|saved| saved.level < UNDERWORLD_LEVEL);
                *visibility = if below && !choice.importing {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
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
/// as the original's edit control's; the Underworld campaign's plain field shows none (seen).
fn caret(
    time: Res<Time<Real>>,
    (choice, screen): (Res<Choice>, Option<Res<State<Screen>>>),
    mut shown: Local<Option<bool>>,
    mut parts: Query<(&Part, &mut Text)>,
) {
    let editable = screen.is_some_and(|screen| *screen.get() == Screen::Heroes);
    let on = editable && (time.elapsed().as_millis() / BLINK).is_multiple_of(2);
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
        let scene = gltf.scenes.first().cloned();
        let idle = gltf.named_animations.get(figure.idle.as_str()).cloned();
        if let (Some(scene), Some(source)) = (scene, figure.gltf.take()) {
            commands
                .entity(entity)
                .insert((WorldAssetRoot(scene), crate::models::Source(source)));
        }
        if let Some(idle) = idle {
            commands.entity(entity).insert(Idle(idle));
        }
    }
}

/// A figure's idle clip, which its animation player plays once it is made.
#[derive(Component)]
struct Idle(Handle<AnimationClip>);

/// Puts the figures' scene's entities into the figures' scene, plays a figure's idle clip as its
/// player comes, and gives its meshes copies of their materials of its own, over their items'
/// effects where they have one.
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
    meshes: Query<
        (
            &MeshMaterial3d<StandardMaterial>,
            Option<&crate::models::Effect>,
        ),
        Without<Owned>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
    (mut graphs, mut making): (ResMut<Assets<AnimationGraph>>, model_effects::Making),
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
            let Ok((material, effect)) = meshes.get(entity) else {
                continue;
            };
            let Some(own) = materials.get(&material.0).cloned() else {
                continue;
            };
            let (colour, reflectance) = (own.base_color, own.reflectance);
            // A mesh over an effect waits for its textures, drawn as it is meanwhile.
            if let Some(effect) = effect {
                let Some(own) = making.over(own, effect) else {
                    continue;
                };
                shades
                    .materials
                    .push((Shaded::Over(own.clone()), colour, reflectance));
                commands
                    .entity(entity)
                    .remove::<MeshMaterial3d<StandardMaterial>>()
                    .insert((MeshMaterial3d(own), Owned));
            } else {
                let own = materials.add(own);
                shades
                    .materials
                    .push((Shaded::Plain(own.clone()), colour, reflectance));
                commands.entity(entity).insert((MeshMaterial3d(own), Owned));
            }
            shades.lit = None;
        }
    }
}

/// The class of the figure that the cursor rests on speaks its introduction once the cursor has
/// rested there half a second, at the speech's volume, in the middle and at its sample's rate;
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
    let hovered = choice.class_at(choice.hovered);
    if hovered != introduction.class {
        if introduction.spoken {
            channel.stop();
        }
        *introduction = Introduction {
            class: hovered,
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
    mut aparts: ResMut<Assets<model_effects::ApartMaterial>>,
) {
    for (figure, mut shades) in &mut figures {
        let lit = choice.hovered == Some(figure.pedestal) || choice.chosen == Some(figure.pedestal);
        if shades.lit == Some(lit) {
            continue;
        }
        shades.lit = Some(lit);
        // A dark figure reflects no light either, which would leave it grey.
        let shaded = if lit { None } else { Some((DARK, 0.0)) };
        for (handle, colour, reflectance) in &shades.materials {
            let (colour, reflectance) = shaded.unwrap_or((*colour, *reflectance));
            match handle {
                Shaded::Plain(handle) => {
                    if let Some(mut material) = materials.get_mut(handle) {
                        (material.base_color, material.reflectance) = (colour, reflectance);
                    }
                }
                Shaded::Over(handle) => {
                    if let Some(mut material) = aparts.get_mut(handle) {
                        (material.base.base_color, material.base.reflectance) =
                            (colour, reflectance);
                    }
                }
            }
        }
    }
}

/// Whether a choice of the hero shows, either campaign's.
fn choosing(screen: Option<Res<State<Screen>>>, exporting: Option<Res<State<Exporting>>>) -> bool {
    screen.is_some_and(|screen| {
        matches!(
            screen.get(),
            Screen::Heroes | Screen::Underworld | Screen::Import
        )
    }) || exported(exporting)
}

/// Whether the screen is a choice of a saved hero.
fn choosing_saved(
    screen: Option<Res<State<Screen>>>,
    exporting: Option<Res<State<Exporting>>>,
) -> bool {
    screen.is_some_and(|screen| matches!(screen.get(), Screen::Underworld | Screen::Import))
        || exported(exporting)
}

/// Whether the export shows over a game.
fn exported(exporting: Option<Res<State<Exporting>>>) -> bool {
    exporting.is_some_and(|exporting| *exporting.get() == Exporting::Shown)
}

/// A choice's screen under its [`Veil`] while what it draws loads: its pictures, its fonts, its
/// figures' models, their items and their materials (#2490), and the frames it has stood ready,
/// which let the figures' pipelines compile while it is covered; and the seconds it has waited.
#[derive(Component, Default)]
struct Veiled {
    ready: u32,
    waited: f32,
}

/// The black over a choice's screen while it loads ([`Veiled`]).
#[derive(Component)]
struct Veil;

/// The veil's layer: over the screen, under the cursor (`cursors`, 10).
const VEIL_LAYER: i32 = 9;

/// The frames a choice's screen stands ready before it shows, and the seconds it waits at the
/// most.
const READY_FRAMES: u32 = 3;
const MOST_WAIT: f32 = 5.0;

/// Shows a choice's screen whole, once all it draws has loaded (or after [`MOST_WAIT`] seconds),
/// where it filled piece by piece as its pictures, its figures and their items came: the
/// original loads the screen before it shows it.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its queries one by one, with their filters"
)]
fn unveil(
    mut commands: Commands,
    (mut roots, veils): (Query<(Entity, &mut Veiled)>, Query<Entity, With<Veil>>),
    (time, images, typefaces): (Res<Time<Real>>, Res<Assets<Image>>, Res<Assets<Font>>),
    children: Query<&Children>,
    (pictures, fonts): (Query<&ImageNode>, Query<&TextFont>),
    figures: Query<(Entity, &Figure, Option<&npcs::Equipment>)>,
    (scenes, retextures): (
        Query<(), (With<WorldAssetRoot>, Without<Children>)>,
        Query<&npcs::Retexture>,
    ),
    unsettled: Query<(), (With<MeshMaterial3d<StandardMaterial>>, Without<Owned>)>,
) {
    // A picture or a text of the screen whose image or font is not there yet: loaded, or made
    // by the screen itself (its water's).
    let waits = |entity: Entity| {
        pictures
            .get(entity)
            .is_ok_and(|picture| !images.contains(&picture.image))
            || fonts.get(entity).is_ok_and(|text| match &text.font {
                bevy::text::FontSource::Handle(font) => !typefaces.contains(font),
                _ => false,
            })
    };
    // A figure's scene, item or mesh not in place: a scene not spawned yet, a mesh not given its
    // own material yet (`settle`), an item's texture not loaded.
    let unready = |entity: Entity| {
        scenes.contains(entity)
            || unsettled.contains(entity)
            || retextures
                .get(entity)
                .is_ok_and(|texture| !images.contains(texture.image()))
    };
    for (root, mut veiled) in &mut roots {
        veiled.waited += time.delta_secs();
        let drawn = !children.iter_descendants(root).any(waits);
        let figures_ready = figures.iter().all(|(figure, look, equipment)| {
            look.gltf.is_none()
                && equipment.is_none_or(npcs::Equipment::is_empty)
                && !std::iter::once(figure)
                    .chain(children.iter_descendants(figure))
                    .any(unready)
        });
        if drawn && figures_ready {
            veiled.ready += 1;
        } else {
            veiled.ready = 0;
        }
        if veiled.ready >= READY_FRAMES || veiled.waited >= MOST_WAIT {
            if veiled.ready < READY_FRAMES {
                warn!("choice: the screen shows after {MOST_WAIT} s, not all of it loaded");
            }
            for veil in &veils {
                commands.entity(veil).despawn();
            }
            commands.entity(root).remove::<Veiled>();
        }
    }
}

/// The campaigns' choices of the hero.
pub struct ChoicePlugin;

impl Plugin for ChoicePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "figures.wesl");
        app.add_plugins((
            crate::ui_layers::NativeMaterialPlugin::<Layer>::default(),
            bevy::app::HierarchyPropagatePlugin::<RenderLayers>::new(PostUpdate),
        ))
        .init_resource::<Choice>()
        .init_resource::<Introduction>()
        .add_systems(OnExit(Screen::Heroes), hush)
        .init_resource::<Figures>()
        .init_resource::<Images>()
        .add_systems(OnEnter(Screen::Heroes), (read_figures, screen).chain())
        .add_systems(
            OnEnter(Screen::Underworld),
            (read_figures, saved_screen).chain(),
        )
        .add_systems(
            OnEnter(Screen::Import),
            (read_figures, saved_screen).chain(),
        )
        .add_systems(Update, erase_tip.run_if(choosing_saved))
        .add_systems(OnExit(Screen::Underworld), drop_tips)
        .add_systems(OnExit(Screen::Import), drop_tips)
        .add_systems(
            OnEnter(Exporting::Shown),
            (read_figures, saved_screen).chain(),
        )
        .add_systems(
            Update,
            (
                export_keys.before(crate::pause::toggle),
                idle_paused.after(bevy::animation::advance_animations),
            )
                .run_if(in_state(Exporting::Shown)),
        )
        .add_systems(
            Update,
            (
                (
                    keys.run_if(in_state(Screen::Heroes)),
                    show,
                    caret,
                    colour_labels.run_if(choosing_saved),
                ),
                // The figures, where the game draws models.
                (dress, settle, shade, npcs::equip, npcs::bind_worn)
                    .chain()
                    .run_if(resource_exists::<Assets<Gltf>>),
            )
                .chain()
                .run_if(choosing),
        )
        .add_systems(Update, colour_words.after(show))
        .add_systems(Update, unveil.run_if(choosing))
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
        let class = |chosen: Option<Class>, by| {
            ORDER[step(chosen.and_then(|c| ORDER.iter().position(|&o| o == c)), by)]
        };
        assert_eq!(class(None, 1), Class::Gladiator);
        assert_eq!(class(None, -1), Class::Vampiress);
        assert_eq!(class(Some(Class::Gladiator), 1), Class::Seraphim);
        assert_eq!(class(Some(Class::Gladiator), -1), Class::Vampiress);
        assert_eq!(class(Some(Class::Vampiress), 1), Class::Gladiator);
        assert_eq!(class(Some(Class::Dwarf), 1), Class::WoodElf);
    }

    #[test]
    fn a_saved_hero_of_the_underworlds_level_is_chosen_with_its_name_and_difficulties() {
        let words = Words::default();
        let saved = |level, highest| Stand {
            class: Class::Dwarf,
            saved: Some(Saved {
                name: "Hero".into(),
                level,
                highest,
                exported: None,
            }),
        };
        let mut choice = Choice::default();
        choice.stands[6] = Some(saved(29, highest_won(&[false; 6])));
        choice.stands[0] = Some(saved(24, 0));
        choice.stands[1] = Some(saved(
            25,
            highest_won(&[true, true, false, false, false, false]),
        ));
        // A free pedestal and a hero below level 25 are not chosen (`0x70CA30`).
        choose(&mut choice, &words, 3);
        choose(&mut choice, &words, 0);
        assert_eq!(choice.chosen, None);
        // A hero that won nothing is offered bronze and silver; one that won silver, gold too.
        choose(&mut choice, &words, 6);
        assert_eq!(choice.chosen, Some(6));
        assert_eq!(choice.name, "Hero");
        assert_eq!(choice.offered, [Difficulty::Bronze, Difficulty::Silver]);
        choose(&mut choice, &words, 1);
        assert_eq!(choice.offered.len(), 3);
        // Niobium is the last, however high the hero won.
        assert_eq!(offered_to(highest_won(&[true; 6])), Difficulty::ALL);
    }

    #[test]
    fn a_saved_hero_starts_in_its_classs_tree_of_the_add_on() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<AppState>()
            .init_resource::<Request>();
        let mut choice = Choice::default();
        choice.stands[6] = Some(Stand {
            class: Class::Dwarf,
            saved: Some(Saved {
                name: "Hero".into(),
                level: 29,
                highest: 0,
                exported: None,
            }),
        });
        choose(&mut choice, &Words::default(), 6);
        choice.difficulty = Difficulty::Silver;
        app.insert_resource(choice);
        app.world_mut().run_system_cached(start).unwrap();
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Loading
        );
        let request = &app.world().resource::<Request>().0;
        assert_eq!(request.class, Some(Class::Dwarf));
        assert_eq!(request.name.as_deref(), Some("Hero"));
        assert_eq!(request.difficulty, Some(Difficulty::Silver));
        assert_eq!(request.tree.as_deref(), Some("addon-type_npc_zwerg"));
        assert_eq!(request.saved, Some(6));
    }

    #[test]
    fn an_imported_hero_of_any_level_starts_in_the_ancaria_campaign_with_its_character() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<AppState>()
            .init_resource::<Request>();
        let exported = |level| Stand {
            class: Class::WoodElf,
            saved: Some(Saved {
                name: "Elda".into(),
                level,
                highest: 0,
                exported: Some(2),
            }),
        };
        // The Underworld campaign's choice takes no hero below level 25, the import any.
        let mut choice = Choice::default();
        choice.stands[2] = Some(exported(3));
        choose(&mut choice, &Words::default(), 2);
        assert_eq!(choice.chosen, None);
        let mut choice = Choice {
            importing: true,
            ..default()
        };
        choice.stands[2] = Some(exported(3));
        choose(&mut choice, &Words::default(), 2);
        assert_eq!(choice.chosen, Some(2));
        // The erase button shows for a hero the player exported.
        assert_eq!(chosen_export(&choice), Some(2));
        app.insert_resource(choice);
        app.world_mut().run_system_cached(start).unwrap();
        app.update();
        let request = &app.world().resource::<Request>().0;
        assert_eq!(request.class, Some(Class::WoodElf));
        assert_eq!(request.name.as_deref(), Some("Elda"));
        // In the class's own tree of the Ancaria campaign, the exported hero's whole character.
        assert_eq!(
            (request.tree.as_deref(), request.saved, request.exported),
            (None, None, Some(2))
        );
    }

    #[test]
    fn the_export_takes_any_pedestal_and_shows_nothing_of_its_hero() {
        let saved = Stand {
            class: Class::Dwarf,
            saved: Some(Saved {
                name: "Hero".into(),
                level: 3,
                highest: 0,
                exported: None,
            }),
        };
        let mut choice = Choice {
            exporting: true,
            ..default()
        };
        choice.stands[6] = Some(saved);
        // A free pedestal, and a saved hero of any level (seen).
        choose(&mut choice, &Words::default(), 2);
        assert_eq!(choice.chosen, Some(2));
        choose(&mut choice, &Words::default(), 6);
        assert_eq!(choice.chosen, Some(6));
        assert!(choice.name.is_empty());
        // The Underworld campaign's choice takes no free pedestal.
        let mut choice = Choice::default();
        choose(&mut choice, &Words::default(), 2);
        assert_eq!(choice.chosen, None);
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
        // The console's key types nothing (#2666).
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Backquote,
            logical_key: Key::Character("`".into()),
            state: bevy::input::ButtonState::Pressed,
            text: Some("`".into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
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
        // The Ancaria campaign's game runs in the class's own tree, of the class's template.
        assert_eq!((request.tree.as_deref(), request.saved), (None, None));
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
