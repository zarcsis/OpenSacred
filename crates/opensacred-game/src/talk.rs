//! The talks (#1028; docs/re/netscript.md, "Dialogues"): a left click on a creature of a dialogue
//! sends the hero within 1.5 times their reach, where the creature stops, both turn to each other
//! and the creature plays its talk; its function `Dialog:<name>` runs (`scripts::speak`), and a
//! window at the screen's middle shows the creature's portrait, name, texts and buttons, laid out
//! and framed as the original's `cUI_Tooltip` (#1458). A button closes it, ends the talk and runs
//! its function (`scripts::answer`); without buttons Enter or a click closes it; the talk ends too
//! where the hero stands farther than 1,500 from the creature. Its voice-over ends as it closes.
//! The world runs on meanwhile, the hero's clicks on it held back while the window shows
//! (docs/decisions/interface.md, "Dialogue window holds back world clicks").

use bevy::picking::Pickable;
use bevy::picking::events::{PointerClick, PointerOut, PointerOver};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::text::{FontSize, LineHeight};
use bevy_kira_audio::prelude::{Audio, AudioControl};
use opensacred_data::tables::PORTRAITS;
use opensacred_rules::minds;

use crate::ai::Mind;
use crate::attack::{self, Aim};
use crate::bar::UiImages;
use crate::fight::Health;
use crate::files;
use crate::frame::{Control, HoldsControl, Playing, Step, Unpaused};
use crate::game::{AppState, FONT};
use crate::hero::{self, Body, Size};
use crate::lines::Texts;
use crate::npcs::{Dies, Npcs, ScriptName, Still, Talk, Talks, Trades, Walk};
use crate::scripts::{self, SHOWN};
use crate::sfx::Sfx;
use crate::voices;

/// How far, past 1.5 times the two creatures' reach, the talk opens (`0x529C60`).
const OPENS_WITHIN: f32 = 1.5;
/// How far apart the hero and the creature end their talk (`0x5370B0`).
const ENDS_BEYOND: f32 = 1_500.0;
/// How far the creature moves from where the hero walks before the hero heads for it again, and
/// how many times the hero stops short of it before it gives the talk up.
const MOVED: f32 = 50.0;
const STOPS: u8 = 3;
/// The sides of the creatures that a type's dialogue of `SetRgnDialog` speaks for (`0x549920`),
/// and the bits of their flags word, one of which they need, and the one they must lack.
const REGION_SIDES: [u8; 6] = [3, 7, 8, 12, 14, 15];
const REGION_FLAGS: u32 = 0x1006_DCF8;
const NOT_REGION: u32 = 0x4_0000;

/// The name's colour (`0xFFFFFF80`), the texts', a button's and one under the cursor, and the
/// window's ground, black at 160/255 (read off the original's frames, #1458), each in the frame's
/// sRGB values (`hud`; #1467).
const NAME: Color = Color::linear_rgb(1.0, 1.0, 128.0 / 255.0);
const WORDS: Color = Color::WHITE;
const BUTTON: Color = Color::linear_rgb(0.0, 1.0, 0.0);
const BUTTON_LIT: Color = Color::linear_rgb(1.0, 1.0, 0.0);
const PANEL: Color = Color::linear_rgba(0.0, 0.0, 0.0, 160.0 / 255.0);
/// The window's measures in the original's pixels (`cUI_Tooltip`'s layout, `0x6E7AF0`): its
/// width, a portrait's room beside the text past the portrait's own width, the portrait's inset
/// from the corner, the text's width (the window's less 16) and its inset, a line, and the room
/// a line takes at the text's foot and at the buttons' row (#1458).
const WIDTH: f32 = 640.0;
const BESIDE_PORTRAIT: f32 = 16.0;
const PORTRAIT_IN: f32 = 8.0;
const TEXT_WIDTH: f32 = 624.0;
const TEXT_IN: f32 = 4.0;
const LINE: f32 = 25.0;
const FOOT: f32 = 4.0;
/// How far below its line's top the original sets a line's letters past where Bevy sets them.
const DROP: f32 = 4.0;
/// The size of the window's font, AntiquaSSK, at which a long line is as wide as the original's
/// (its GDI's size is not read): its hinted glyphs of whole pixels make a line of narrow ones
/// wider, so that such a line may take a word more here (docs/decisions/interface.md).
const FONT_SIZE: f32 = 23.7;
/// The frame about the window, its `UI_STATIC_BORDER`s: the interface's images of an edge
/// across, an edge down and a corner (`0x9ECEB0`'s 21, 34 and 55), each edge stretched along
/// its side, the corner as it is at the top right and turned about for the others.
const EDGE_ACROSS: u32 = 21;
const EDGE_DOWN: u32 = 34;
const CORNER: u32 = 55;
/// An `InfoPlayer`'s text: its colour, as the chapters' titles show, how long it shows, in
/// seconds, and how far down the screen (**hypothesis**: the engine's event `0x10` of the kind 3
/// is not read).
const NOTICE: Color = Color::linear_rgb(1.0, 0.55, 0.12);
const NOTICE_SIZE: f32 = 25.2;
const NOTICE_SECONDS: f32 = 5.0;
const NOTICE_TOP: f32 = 22.0;
/// The modes of an `InfoPlayer` that show its text (`0x4AE350`): 0 and 1, 2 with the music
/// `0x1B61` once, which has no file, and 3 with the sound 7000, which no tree's `InfoPlayer` asks
/// for (#1243).
const SHOWS: [i32; 4] = [0, 1, 2, 3];

/// An `InfoPlayer`'s text on the screen, and how long it still shows.
#[derive(Component)]
struct Notice(f32);

/// What a talk opens (`0x52AB70`): the creature's dialogue by its function's name, or, for a
/// combat master without one (its number below 1 or above 200,000), the master's window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Speech {
    Dialogue(String),
    Master,
}

impl std::fmt::Display for Speech {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dialogue(dialogue) => f.write_str(dialogue),
            Self::Master => f.write_str("the combat master's window"),
        }
    }
}

/// What the creature of `talk`, `mind` and `trades` opens as the hero speaks to it, of `npcs`:
/// its dialogue ([`dialogue_of`]), else the master's window where it is a combat master
/// (`+0x200` bit `0x4000`, `quests::WIZARD`); the smiths' and the merchants' windows are not
/// followed.
pub(crate) fn speech_of(
    talk: Option<&Talk>,
    mind: &Mind,
    trades: Option<&Trades>,
    npcs: Option<&Npcs>,
) -> Option<Speech> {
    dialogue_of(talk, mind, npcs)
        .map(Speech::Dialogue)
        .or_else(|| {
            trades
                .is_some_and(|trades| trades.0 & opensacred_rules::quests::WIZARD != 0)
                .then_some(Speech::Master)
        })
}

/// The hero speaking to `creature`, of `dialogue`: going to it until the talk opens.
#[derive(Component, Clone, Debug)]
pub struct Talking {
    creature: Entity,
    dialogue: Speech,
    open: bool,
    /// How many times the hero has stopped short of the creature.
    stops: u8,
}

/// The creatures' portraits of the dialogues' window (#1032): the table of `tables/portraits.ron`
/// and its textures' files by their names; none without the table (assets converted before it).
#[derive(Resource, Default)]
pub struct Portraits {
    table: Option<opensacred_data::tables::Portraits>,
    files: std::collections::HashMap<String, String>,
}

impl Portraits {
    /// Reads `tables/portraits.ron` of `assets` and finds its textures.
    pub fn read(assets: &std::path::Path) -> anyhow::Result<Self> {
        let path = assets.join("tables").join(PORTRAITS);
        if !files::exists(&path) {
            return Ok(Self::default());
        }
        let text = files::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
        let table: opensacred_data::tables::Portraits =
            ron::from_str(&text).map_err(|e| anyhow::anyhow!("parsing {}: {e}", path.display()))?;
        let mut names: Vec<&str> = table
            .creatures
            .iter()
            .map(|c| c.portrait.texture.as_str())
            .chain([table.default.texture.as_str()])
            .collect();
        names.sort_unstable();
        names.dedup();
        let found = crate::hud::find_textures(assets, &names);
        let files = names
            .into_iter()
            .zip(found)
            .filter_map(|(name, file)| Some((name.to_owned(), file?)))
            .collect();
        Ok(Self {
            table: Some(table),
            files,
        })
    }

    /// The texture's file of the creature type `creature`'s portrait, and its part: its left,
    /// top, width and height.
    pub(crate) fn of(&self, creature: u32) -> Option<(&str, [u16; 4])> {
        let portrait = self.table.as_ref()?.of(creature);
        Some((self.files.get(&portrait.texture)?.as_str(), portrait.rect))
    }

    /// The interface's image of the creature type `creature` in the portraits' column
    /// (`0x435D30`, #1465), none for a type without one or without the table's column (assets
    /// converted before it).
    pub(crate) fn column(&self, creature: u32) -> Option<u32> {
        self.table.as_ref()?.column_image(creature)
    }
}

/// A portrait in the window: its texture, and its part, its left, top, width and height.
type Portrait = (Handle<Image>, [u16; 4]);

/// The dialogue's window while it shows: its root, the creature, and its buttons' functions.
#[derive(Resource, Clone, Debug)]
pub struct Shown {
    root: Entity,
    creature: Entity,
    functions: Vec<String>,
    /// Its name and texts, and its buttons' labels, which the command `answer` tells.
    text: String,
    labels: Vec<String>,
}

/// The dialogue a creature speaks, its own (`+0x245`, [`Talk`]) or, without one, its type's of
/// `SetRgnDialog` where it is of the townsfolk's sides and flags and hostile neither way
/// (`0x549920`); found anew at each query.
pub(crate) fn dialogue_of(talk: Option<&Talk>, mind: &Mind, npcs: Option<&Npcs>) -> Option<String> {
    let region = |creature| npcs?.region_talk(creature).map(str::to_owned);
    dialogue_by(talk, mind, region)
}

/// [`dialogue_of`], its type's dialogue of `SetRgnDialog` by `region`.
fn dialogue_by(
    talk: Option<&Talk>,
    mind: &Mind,
    region: impl FnOnce(u32) -> Option<String>,
) -> Option<String> {
    if let Some(talk) = talk {
        return Some(talk.0.clone());
    }
    let townsfolk = REGION_SIDES.contains(&mind.side)
        && mind.flags & REGION_FLAGS != 0
        && mind.flags & NOT_REGION == 0
        && !mind.hostile();
    townsfolk.then(|| region(mind.creature)).flatten()
}

/// On a press of the left button, the creature of a dialogue alive nearest the cursor's line of
/// sight becomes the hero's to speak to, the hero walking to it, ahead of an item to take
/// (`0x53A170`); any other press ends a talk not open yet.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their \
              filters"
)]
fn pick(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    pointer: Res<crate::hud::Pointer>,
    mut aim: ResMut<Aim>,
    camera: Single<(&Camera, &GlobalTransform), crate::iso::Eye<With<Camera>>>,
    npcs: Option<Res<Npcs>>,
    creatures: Query<
        (
            Entity,
            &GlobalTransform,
            &Mind,
            &Health,
            Option<&Talk>,
            Option<&Trades>,
            &crate::crowd::InTheWay,
        ),
        Without<Dies>,
    >,
    mut heroes: Query<(Entity, &mut Body, Option<&Talking>)>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    for (hero, _, talking) in &heroes {
        if talking.is_some_and(|talking| !talking.open) {
            commands.entity(hero).remove::<Talking>();
        }
    }
    let Some(cursor) = pointer.0 else {
        return;
    };
    let (camera, view) = *camera;
    let Ok(ray) = camera.viewport_to_world(view, cursor) else {
        return;
    };
    let Some((creature, (at, level), dialogue)) = creatures
        .iter()
        .filter(|(_, _, _, health, ..)| health.current > 0)
        .filter_map(|(entity, at, mind, _, talk, trades, way)| {
            let dialogue = speech_of(talk, mind, trades, npcs.as_deref())?;
            let at = at.translation();
            Some((
                entity,
                (at, way.level),
                dialogue,
                attack::from_line(ray, at),
            ))
        })
        .filter(|&(.., distance)| distance <= attack::PICKED_WITHIN)
        .min_by(|a, b| a.3.total_cmp(&b.3))
        .map(|(entity, at, dialogue, _)| (entity, at, dialogue))
    else {
        return;
    };
    aim.0 = true;
    for (hero, mut body, _) in &mut heroes {
        debug!("talk: the hero goes to speak to {creature} ({dialogue})");
        body.send_onto(at, level);
        // The strike goes with its attack: `attack::engage`, which alone ends it, takes only a
        // hero that attacks (#1215).
        commands
            .entity(hero)
            .remove::<(attack::Attack, attack::Striking)>()
            .insert(Talking {
                creature,
                dialogue: dialogue.clone(),
                open: false,
                stops: 0,
            });
    }
}

/// The hero going to speak to a creature: within 1.5 times their reach it stops, the creature
/// stops, both turn to each other and the creature plays its talk, and the dialogue runs
/// ([`open`]); farther it walks to the creature, heading for it again as it moves, and gives the
/// talk up where it stops short of it again and again, or where the creature is gone or dead
/// (`0x529C60`, `0x52AB70`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn approach(
    mut commands: Commands,
    mut heroes: Query<(
        Entity,
        &mut Transform,
        &mut Body,
        &mut Talking,
        Option<&Size>,
    )>,
    mut creatures: Query<
        (&mut Transform, &Mind, &Health, &crate::crowd::InTheWay),
        (Without<Body>, Without<Dies>),
    >,
) {
    for (hero, mut transform, mut body, mut talking, size) in &mut heroes {
        if talking.open {
            continue;
        }
        let Ok((mut theirs, mind, health, way)) = creatures.get_mut(talking.creature) else {
            commands.entity(hero).remove::<Talking>();
            continue;
        };
        if health.current == 0 {
            commands.entity(hero).remove::<Talking>();
            continue;
        }
        let here = transform.translation;
        let there = theirs.translation;
        let size = size.map_or(minds::SIZE, |size| size.0);
        let within = OPENS_WITHIN * minds::reach_f32(size, mind.size());
        if (there - here).xz().length() > within {
            // An order waits for the world's next tick (`hero::take_orders`).
            if body.ordered() {
                continue;
            }
            let heading = body.target();
            if heading.is_none() {
                talking.stops += 1;
                if talking.stops > STOPS {
                    debug!("talk: the hero stands too far from {}", talking.creature);
                    commands.entity(hero).remove::<Talking>();
                    continue;
                }
            }
            if heading.is_none_or(|to| (to - there).xz().length() > MOVED) {
                body.send_onto(there, way.level);
            }
            continue;
        }
        body.stop();
        transform.rotation = hero::facing(there - here);
        theirs.rotation = hero::facing(here - there);
        talking.open = true;
        let creature = talking.creature;
        commands
            .entity(creature)
            .remove::<Walk>()
            .insert((Still, Talks));
        match talking.dialogue.clone() {
            Speech::Dialogue(dialogue) => {
                commands.queue(move |world: &mut World| open(world, hero, creature, &dialogue));
            }
            // A master shows its window beside the inventory, and the talk ends at once
            // (`0x52AB70`).
            Speech::Master => {
                commands.queue(move |world: &mut World| {
                    crate::master::open(world, creature);
                    end(world, hero, creature);
                });
            }
        }
    }
}

/// Runs the creature's dialogue (`scripts::speak`) and shows its window: its name, its texts and
/// its first four buttons, its sounds played; a dialogue that takes the creature's away
/// (`DelDlg`) takes it; one without a text ends the talk at once.
fn open(world: &mut World, hero: Entity, creature: Entity, dialogue: &str) {
    let window = scripts::speak(world, dialogue).unwrap_or_default();
    if window.untalks
        && let Ok(mut entity) = world.get_entity_mut(creature)
    {
        entity.remove::<Talk>();
        if let Some(mut mind) = entity.get_mut::<Mind>() {
            mind.dialogue = false;
        }
    }
    let texts = world.get_resource::<Texts>();
    let lines: Vec<String> = window
        .texts
        .iter()
        .filter_map(|key| texts.and_then(|texts| texts.named(key)).map(shown_text))
        .collect();
    if lines.is_empty() {
        info!("talk: {dialogue} shows no text");
        end(world, hero, creature);
        return;
    }
    let name = world
        .get::<ScriptName>(creature)
        .and_then(|name| texts.and_then(|texts| texts.named(resource(&name.0))))
        .map(str::to_owned);
    let buttons: Vec<(String, String)> = window
        .buttons
        .iter()
        .take(SHOWN)
        .filter_map(|(label, function)| {
            let label = texts.and_then(|texts| texts.named(resource(label)))?;
            Some((label.to_owned(), function.clone()))
        })
        .collect();
    info!(
        "talk: {dialogue} shows {} texts and {} buttons",
        lines.len(),
        buttons.len()
    );
    voice(world, &window.sounds);
    let font: Handle<Font> = world.resource::<AssetServer>().load(FONT);
    // Its portrait by its type (`0x4345C0`, #1032); the hero's own talk shows none.
    let portrait = world
        .get::<Mind>(creature)
        .map(|mind| mind.creature)
        .and_then(|kind| {
            let (file, rect) = world.get_resource::<Portraits>()?.of(kind)?;
            Some((file.to_owned(), rect))
        })
        .map(|(file, rect)| {
            let image = crate::hud::image(world.resource::<AssetServer>(), file);
            (image, rect)
        });
    let functions = buttons.iter().map(|(_, f)| f.clone()).collect();
    let text = lines.join("\n");
    let width = WIDTH + beside(portrait.as_ref());
    let frame = world
        .get_resource::<UiImages>()
        .map(|images| frame(images, world.resource::<AssetServer>(), width))
        .unwrap_or_default();
    let root = spawn_window(
        &mut world.commands(),
        &font,
        (name.as_deref(), portrait),
        (&text, &buttons),
        frame,
    );
    world.flush();
    world.insert_resource(Shown {
        root,
        creature,
        functions,
        text: name.map_or(text.clone(), |name| format!("{name}\n\n{text}")),
        labels: buttons.into_iter().map(|(label, _)| label).collect(),
    });
}

/// A text of `global.res` as the window shows it: its line breaks (`<n>`, 3,760 of them in the
/// English texts) broken, its colours' tags (`<color=…>`, `<colorw=…>`) left out, and any other
/// tag, a speaker's sigh or groan, kept as it stands.
pub(crate) fn shown_text(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let tag = &rest[start..];
        let Some(end) = tag.find('>') else {
            out.push_str(tag);
            return out;
        };
        match &tag[1..end] {
            "n" => out.push('\n'),
            inner if inner.starts_with("color") => {}
            _ => out.push_str(&tag[..=end]),
        }
        rest = &tag[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The key of a name of the scripts' resources, `res:<key>` (case aside), as the window looks
/// its string up: `res:17095` the string of `17095`.
pub(crate) fn resource(name: &str) -> &str {
    match name.get(..4) {
        Some(prefix) if prefix.eq_ignore_ascii_case("res:") => &name[4..],
        _ => name,
    }
}

/// Plays the dialogue's sounds, its voice-overs under their texts' keys (`sound.pak`'s
/// `SOUND_FX_<key>`, `0x4A9730`), each the line by name that stops the one before, the hero's
/// own too (`0x693FE0`, `voices::NamedLine`).
fn voice(world: &mut World, sounds: &[String]) {
    for sound in sounds {
        let name = format!("SOUND_FX_{}", sound.to_ascii_uppercase());
        let Some((id, file)) = world.get_resource::<Sfx>().and_then(|sfx| sfx.named(&name)) else {
            debug!("talk: no sound {name}");
            continue;
        };
        let assets = world.resource::<AssetServer>().clone();
        let Some(mut sfx) = world.get_resource_mut::<Sfx>() else {
            return;
        };
        let sample = sfx.sample(id, file, &assets);
        let Some(audio) = world.get_resource::<Audio>() else {
            return;
        };
        let instance = audio.play(sample).handle();
        voices::name_line(world, Some(instance));
    }
}

/// Stops the line by name that plays, the window's voice-over or the hero's own line
/// (`0x693FB0`).
fn hush(world: &mut World) {
    voices::name_line(world, None);
}

/// The room a portrait takes beside the window's text, which widens the window: its width and 16
/// (`0x6E7AF0`); none without one.
fn beside(portrait: Option<&Portrait>) -> f32 {
    portrait.map_or(0.0, |(_, [_, _, w, _])| f32::from(*w) + BESIDE_PORTRAIT)
}

/// The pieces of the window's frame (#1458) about a window `width` wide: the edges across at the
/// top and, turned about, at the bottom, the edges down at the sides, and the corners, each image
/// on its node, placed about the window's rectangle as the original's frames show them; each
/// node's size given, as an image's node along an edge would keep its picture's size.
pub(crate) fn frame(images: &UiImages, assets: &AssetServer, width: f32) -> Vec<(ImageNode, Node)> {
    let at = |[left, right, top, bottom]: [Option<f32>; 4], (width, height): (Val, Val)| Node {
        position_type: PositionType::Absolute,
        left: left.map_or(Val::Auto, Val::Px),
        right: right.map_or(Val::Auto, Val::Px),
        top: top.map_or(Val::Auto, Val::Px),
        bottom: bottom.map_or(Val::Auto, Val::Px),
        width,
        height,
        ..default()
    };
    let across = (Val::Px(width - 4.0), Val::Px(images.size(EDGE_ACROSS).y));
    let down = images.size(EDGE_DOWN).x;
    let side = (Val::Px(down), Val::Percent(100.0));
    let corner = images.size(CORNER);
    let corner = (Val::Px(corner.x), Val::Px(corner.y));
    [
        (
            EDGE_ACROSS,
            [false; 2],
            at([Some(4.0), None, Some(-4.0), None], across),
        ),
        (
            EDGE_ACROSS,
            [false, true],
            at([Some(4.0), None, None, Some(-3.0)], across),
        ),
        (
            EDGE_DOWN,
            [false; 2],
            at([Some(-4.0), None, Some(0.0), None], side),
        ),
        (
            EDGE_DOWN,
            [false; 2],
            at([None, Some(-down), Some(0.0), None], side),
        ),
        (
            CORNER,
            [true, false],
            at([Some(-13.0), None, Some(-11.0), None], corner),
        ),
        (
            CORNER,
            [false; 2],
            at([None, Some(-14.0), Some(-11.0), None], corner),
        ),
        (
            CORNER,
            [true, true],
            at([Some(-13.0), None, None, Some(-11.0)], corner),
        ),
        (
            CORNER,
            [false, true],
            at([None, Some(-14.0), None, Some(-11.0)], corner),
        ),
    ]
    .into_iter()
    .filter_map(|(index, [flip_x, flip_y], node)| {
        let (file, rect, [turned_x, turned_y]) = images.turned_part(index)?;
        let image = ImageNode {
            rect: Some(rect),
            flip_x: flip_x != turned_x,
            flip_y: flip_y != turned_y,
            image_mode: NodeImageMode::Stretch,
            ..ImageNode::new(crate::hud::image(assets, file.to_owned()))
        };
        Some((image, node))
    })
    .collect()
}

/// The window, as the original's `cUI_Tooltip` lays a dialogue out (`0x6E7AF0`, `0x6E76C0`;
/// #1458): 640 wide and as high as its lines, at the screen's middle, black at 160/255 within
/// its frame; the portrait 8 in from its corner and the text beside it, 16 past the portrait's
/// width, the window wider by as much; the name and, a line apart, the texts in one justified
/// block of lines of 25, at least as high as the portrait; under it a row of the buttons, the
/// window's width shared out evenly about them, green, yellow under the cursor, each choosing
/// its function ([`choose`]).
fn spawn_window(
    commands: &mut Commands,
    font: &Handle<Font>,
    (name, portrait): (Option<&str>, Option<Portrait>),
    (text, buttons): (&str, &[(String, String)]),
    frame: Vec<(ImageNode, Node)>,
) -> Entity {
    let words = (
        TextFont {
            font: font.clone().into(),
            font_size: FontSize::Px(FONT_SIZE),
            ..default()
        },
        LineHeight::Px(LINE),
    );
    let beside = beside(portrait.as_ref());
    let tallest = portrait
        .as_ref()
        .map_or(0.0, |(_, [_, _, _, h])| f32::from(*h));
    commands
        .spawn((
            DespawnOnExit(AppState::InGame),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Node {
                        width: Val::Px(WIDTH + beside),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    BackgroundColor(PANEL),
                ))
                .with_children(|panel| {
                    if let Some((image, [x, y, w, h])) = portrait {
                        let (x, y, w, h) = (f32::from(x), f32::from(y), f32::from(w), f32::from(h));
                        panel.spawn((
                            ImageNode {
                                image,
                                rect: Some(Rect::new(x, y, x + w, y + h)),
                                ..default()
                            },
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(PORTRAIT_IN),
                                top: Val::Px(PORTRAIT_IN),
                                width: Val::Px(w),
                                height: Val::Px(h),
                                ..default()
                            },
                        ));
                    }
                    panel
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            width: Val::Px(TEXT_WIDTH),
                            min_height: Val::Px(tallest - FOOT),
                            margin: UiRect {
                                left: Val::Px(beside + TEXT_IN),
                                top: Val::Px(DROP),
                                bottom: Val::Px(FOOT - DROP),
                                ..default()
                            },
                            ..default()
                        })
                        .with_children(|block| {
                            // The name, a line, then the texts (`0x6E70D0`).
                            if let Some(name) = name {
                                block.spawn((
                                    Text::new(name),
                                    words.clone(),
                                    TextColor(NAME),
                                    Node {
                                        margin: UiRect::bottom(Val::Px(LINE)),
                                        ..default()
                                    },
                                ));
                            }
                            block.spawn((
                                Text::new(text),
                                words.clone(),
                                TextColor(WORDS),
                                TextLayout::justify(Justify::Justified),
                            ));
                        });
                    if !buttons.is_empty() {
                        panel
                            .spawn(Node {
                                height: Val::Px(LINE + FOOT),
                                padding: UiRect::top(Val::Px(DROP)),
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceEvenly,
                                ..default()
                            })
                            .with_children(|row| {
                                for (index, (label, _)) in buttons.iter().enumerate() {
                                    spawn_button(row, (label, index), &words);
                                }
                            });
                    }
                    for piece in frame {
                        panel.spawn((piece, Pickable::IGNORE));
                    }
                });
        })
        .id()
}

/// A button of the window's row: its label, green and yellow under the cursor, and a click on it
/// choosing its function.
fn spawn_button(
    row: &mut ChildSpawnerCommands,
    (label, index): (&str, usize),
    words: &(TextFont, LineHeight),
) {
    row.spawn((Text::new(label), words.clone(), TextColor(BUTTON)))
        .observe(
            |over: On<PointerOver>, mut colours: Query<&mut TextColor>| {
                if let Ok(mut colour) = colours.get_mut(over.entity) {
                    colour.0 = BUTTON_LIT;
                }
            },
        )
        .observe(|out: On<PointerOut>, mut colours: Query<&mut TextColor>| {
            if let Ok(mut colour) = colours.get_mut(out.entity) {
                colour.0 = BUTTON;
            }
        })
        .observe(move |click: On<PointerClick>, mut commands: Commands| {
            if click.button == PointerButton::Primary {
                commands.queue(move |world: &mut World| {
                    choose(world, index);
                });
            }
        });
}

/// A button chosen: the window closes, the talk ends, and its function runs with no creature
/// (`0x6E74E0`, `0x499DA0`).
fn choose(world: &mut World, index: usize) {
    let Some(shown) = world.get_resource::<Shown>().cloned() else {
        return;
    };
    close(world);
    if let Some(function) = shown.functions.get(index) {
        info!("talk: the button {index} runs {function}");
        scripts::answer(world, function);
    }
}

/// Closes the window, if one shows, its voice-over stopped, and ends the talk.
fn close(world: &mut World) {
    let Some(shown) = world.remove_resource::<Shown>() else {
        return;
    };
    hush(world);
    if let Ok(root) = world.get_entity_mut(shown.root) {
        root.despawn();
    }
    let mut heroes = world.query_filtered::<Entity, With<Talking>>();
    let heroes: Vec<Entity> = heroes.iter(world).collect();
    for hero in heroes {
        end(world, hero, shown.creature);
    }
}

/// Ends the talk of `hero` with `creature`: the creature free to go again.
fn end(world: &mut World, hero: Entity, creature: Entity) {
    if let Ok(mut hero) = world.get_entity_mut(hero) {
        hero.remove::<Talking>();
    }
    if let Ok(mut creature) = world.get_entity_mut(creature) {
        creature.remove::<(Still, Talks)>();
    }
}

/// Closes a window without buttons on Enter or a press of a mouse button (`0x655920`), and any
/// window once the hero and the creature stand farther than 1,500 apart or the creature is gone
/// (`0x5370B0`).
fn dismiss(
    mut commands: Commands,
    shown: Option<Res<Shown>>,
    keys: crate::controls::Keys,
    buttons: Res<ButtonInput<MouseButton>>,
    heroes: Query<&Transform, With<Body>>,
    creatures: Query<&Transform, Without<Dies>>,
) {
    let Some(shown) = shown else {
        return;
    };
    let pressed = shown.functions.is_empty()
        && (keys.just_pressed(crate::controls::Action::Confirm)
            || buttons.get_just_pressed().next().is_some());
    let apart = match (heroes.iter().next(), creatures.get(shown.creature)) {
        (Some(hero), Ok(creature)) => {
            (creature.translation - hero.translation).xz().length() > ENDS_BEYOND
        }
        _ => true,
    };
    if pressed || apart {
        commands.queue(close);
    }
}

/// Shows the scripts' `InfoPlayer`s' texts (#1033): of a mode that shows one, the last of the
/// frame's, in place of the one before, across the screen's upper part, for 5 s; the other modes
/// (an image, an event, the interface's states) are not followed.
fn notices(
    mut commands: Commands,
    mut infos: ResMut<scripts::Infos>,
    texts: Option<Res<Texts>>,
    assets: Res<AssetServer>,
    shown: Query<Entity, With<Notice>>,
) {
    let mut last = None;
    for (mode, key) in infos.0.drain(..) {
        if !SHOWS.contains(&mode) {
            debug!("talk: an InfoPlayer of the mode {mode} ({key}) is not followed");
            continue;
        }
        match texts
            .as_deref()
            .and_then(|texts| texts.named(resource(&key)))
        {
            Some(text) => last = Some(shown_text(text)),
            None => debug!("talk: no text {key}"),
        }
    }
    let Some(text) = last else {
        return;
    };
    info!(
        "talk: the player is shown {}",
        text.lines().next().unwrap_or_default()
    );
    for old in &shown {
        commands.entity(old).despawn();
    }
    let font: Handle<Font> = assets.load(FONT);
    commands
        .spawn((
            DespawnOnExit(AppState::InGame),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(NOTICE_TOP),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
            Notice(NOTICE_SECONDS),
        ))
        .with_children(|row| {
            row.spawn((
                Text::new(text),
                TextFont {
                    font: font.into(),
                    font_size: FontSize::Px(NOTICE_SIZE),
                    ..default()
                },
                TextColor(NOTICE),
                TextLayout::justify(Justify::Center),
            ));
        });
}

/// Takes an `InfoPlayer`'s text away once its time is over.
fn fade(mut commands: Commands, time: Res<Time>, mut notices: Query<(Entity, &mut Notice)>) {
    for (entity, mut notice) in &mut notices {
        notice.0 -= time.delta_secs();
        if notice.0 <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// Begins the hero's own talk of each dialogue the scripts' `Popup`s give it (`0x4A7760`,
/// `0x54CBB0`; #1033), one at a time, once no window shows: its window without a name.
fn popups(
    mut commands: Commands,
    mut popups: ResMut<scripts::Popups>,
    shown: Option<Res<Shown>>,
    heroes: Query<Entity, With<Body>>,
) {
    if shown.is_some() || popups.0.is_empty() {
        return;
    }
    let Some(hero) = heroes.iter().next() else {
        return;
    };
    match popups.0.remove(0) {
        scripts::Begins::Own(dialogue) => {
            info!("talk: the hero's own talk of {dialogue}");
            commands.queue(move |world: &mut World| open(world, hero, hero, &dialogue));
        }
        // The creature takes the dialogue, and the hero speaks to it as a click on it does
        // (`0x49DCF0`: the hero's talk's action toward it).
        scripts::Begins::With { creature, dialogue } => {
            commands.queue(move |world: &mut World| {
                let mut named = world.query::<(Entity, &crate::npcs::ScriptName)>();
                let found = named
                    .iter(world)
                    .find(|(_, name)| name.0 == creature)
                    .map(|(entity, _)| entity);
                let Some(entity) = found else {
                    info!("talk: no {creature} in the world to speak {dialogue}");
                    return;
                };
                world.entity_mut(entity).insert(Talk(dialogue.clone()));
                match talk_to(world, Some(entity)) {
                    Ok(line) => info!("talk: {creature} takes {dialogue}: {line}"),
                    Err(e) => info!("talk: {creature} takes {dialogue}: {e}"),
                }
            });
        }
    }
}

/// Sends the hero to speak to `creature`, or to the creature of a dialogue or the combat master
/// nearest it (the
/// command `talk`), as a click on it does: a line of which.
pub fn talk_to(world: &mut World, creature: Option<Entity>) -> Result<String, String> {
    let mut heroes = world.query_filtered::<(Entity, &Transform), With<Body>>();
    let (hero, here) = heroes
        .iter(world)
        .next()
        .map(|(hero, transform)| (hero, transform.translation))
        .ok_or("the hero is not in the world yet")?;
    let found: Vec<(Entity, Vec3, crate::terrain::Level, Speech)> = {
        let mut creatures = world.query_filtered::<(
            Entity,
            &Transform,
            &Mind,
            &Health,
            (Option<&Talk>, Option<&Trades>),
            &crate::crowd::InTheWay,
        ), Without<Dies>>();
        let npcs = world.get_resource::<Npcs>();
        creatures
            .iter(world)
            .filter(|(entity, _, _, health, ..)| {
                health.current > 0 && creature.is_none_or(|c| c == *entity)
            })
            .filter_map(|(entity, at, mind, _, (talk, trades), way)| {
                Some((
                    entity,
                    at.translation,
                    way.level,
                    speech_of(talk, mind, trades, npcs)?,
                ))
            })
            .collect()
    };
    let (creature, at, level, dialogue) = found
        .into_iter()
        .min_by(|a, b| {
            let d = |at: Vec3| (at - here).xz().length_squared();
            d(a.1).total_cmp(&d(b.1))
        })
        .ok_or("no creature of a dialogue or combat master is near")?;
    let mut body = world
        .get_mut::<Body>(hero)
        .ok_or("the hero is not in the world yet")?;
    body.send_onto(at, level);
    world
        .entity_mut(hero)
        .remove::<(crate::attack::Attack, crate::attack::Striking)>()
        .insert(Talking {
            creature,
            dialogue: dialogue.clone(),
            open: false,
            stops: 0,
        });
    Ok(format!("the hero goes to speak to {creature} ({dialogue})"))
}

/// Chooses the shown window's button `button` (from 1), as a click on it does, or tells its
/// lines without one (the command `answer`).
pub fn answer(world: &mut World, button: Option<usize>) -> Result<String, String> {
    let shown = world
        .get_resource::<Shown>()
        .cloned()
        .ok_or("no dialogue's window shows")?;
    let Some(button) = button else {
        let mut lines = vec![shown.text.clone()];
        lines.extend(
            shown
                .labels
                .iter()
                .enumerate()
                .map(|(i, label)| format!("{}: {label}", i + 1)),
        );
        return Ok(lines.join("\n"));
    };
    let index = button
        .checked_sub(1)
        .filter(|&i| i < shown.functions.len())
        .ok_or_else(|| format!("the window has {} buttons", shown.functions.len()))?;
    choose(world, index);
    Ok(format!(
        "the button {button} runs {}",
        shown.functions[index]
    ))
}

/// Whether no dialogue's window shows, which holds back the hero's clicks on the world.
fn no_window(shown: Option<Res<Shown>>) -> bool {
    shown.is_none()
}

/// Leaves no window and no voice-over behind a game.
fn forget(mut commands: Commands) {
    commands.remove_resource::<Shown>();
    commands.queue(hush);
}

/// The talks.
pub struct TalkPlugin;

impl Plugin for TalkPlugin {
    fn build(&self, app: &mut App) {
        app.hold_control(no_window)
            .add_systems(OnExit(AppState::InGame), forget)
            .add_systems(
                Update,
                (
                    pick.in_set(Control)
                        .after(attack::aim)
                        .after(hero::steer)
                        .after(crate::doors::pick)
                        .before(crate::pickup::pick),
                    approach.after(hero::run),
                )
                    .in_set(Unpaused)
                    .in_set(Step::HeroMove),
            )
            .add_systems(Update, (dismiss, notices, fade, popups).in_set(Playing));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_creature_speaks_its_own_dialogue_or_its_types_as_townsfolk() {
        let farmer = |side: u8, flags: u32| {
            let mut mind = Mind::new(
                Vec3::ZERO,
                flags,
                (false, true),
                None,
                [70.0, 180.0],
                (677, 3),
            );
            mind.side = side;
            mind
        };
        let region = |creature: u32| (creature == 677).then(|| "NPC_Dialog_Farmer".to_owned());
        let own = Talk("auftrag10".into());
        // Its own first, whatever it is.
        assert_eq!(
            dialogue_by(Some(&own), &farmer(2, 0x1), region).as_deref(),
            Some("auftrag10")
        );
        // A townsperson of the side 3 and the flags 8 takes its type's.
        assert_eq!(
            dialogue_by(None, &farmer(3, 0x8), region).as_deref(),
            Some("NPC_Dialog_Farmer")
        );
        // Not of a hostile side, nor without the flags, nor of the flag 0x40000.
        assert_eq!(dialogue_by(None, &farmer(2, 0x8), region), None);
        assert_eq!(dialogue_by(None, &farmer(3, 0), region), None);
        assert_eq!(dialogue_by(None, &farmer(3, 0x4_0008), region), None);
    }

    #[test]
    fn a_text_breaks_its_lines_and_leaves_its_colours_out() {
        assert_eq!(shown_text("one.<n><n>two"), "one.\n\ntwo");
        assert_eq!(
            shown_text("<colorw=FFFFFF80>name <sigh> on"),
            "name <sigh> on"
        );
        assert_eq!(shown_text("open <n"), "open <n");
    }

    #[test]
    fn a_resource_name_is_looked_up_by_its_key() {
        assert_eq!(resource("res:17095"), "17095");
        assert_eq!(resource("Res:1024"), "1024");
        assert_eq!(resource("RES:HQ_1"), "HQ_1");
        assert_eq!(resource("Dialog"), "Dialog");
        assert_eq!(resource("ré"), "ré");
    }
}
