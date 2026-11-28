//! The talks (#1028; docs/re/netscript.md, "Dialogues"): a left click on a creature of a dialogue
//! stops the hero where it stands within 1.5 times their reach, else sends it within the reach
//! (#1636); there the creature stops, both turn to each other and the creature plays its talk;
//! its function `Dialog:<name>` runs (`scripts::speak`), and a window at the screen's middle
//! shows the creature's portrait, name, texts and buttons, laid out and framed as the original's
//! `cUI_Tooltip` (#1458). A button closes it, ends the talk and runs
//! its function (`scripts::answer`); without buttons Enter or a click closes it; the talk ends too
//! where the hero stands farther than 1,500 from the creature. Its voice-over ends as it closes.
//! The world runs on meanwhile, the hero's clicks on it held back while the window shows, as the
//! original's hero in its talk acts on none (seen, #2877; docs/re/netscript.md, "The talk's own
//! end, and the world meanwhile"), and Esc's menu and P's pause go on over the window.
//!
//! In a dialogue's place a townsperson of the Underworld's regions without one says a generic
//! line in the window of `Dialog:NPC_UW_DLG` ([`Lines`], `opensacred_rules::quests::
//! underworld_line`, #2887), and a talk opens the smith's, the merchant's or the combat master's
//! window for such a creature without a dialogue, and the horse window for a horse trader or a
//! stable's horse ([`Speech`], [`speech_of`]). The scripts begin talks too ([`popups`]: the hero's own of
//! a `Popup`, and one with the creature of an `NPC_TalkTo`), show their `InfoPlayer`s' texts as
//! lines ([`notices`], #1033) and play their `PlaySound`s outside a window ([`lines()`], #1826).
//! The module also reads the creatures' portraits ([`Portraits`], #1032) and carries out the
//! commands `talk` and `answer` ([`talk_to`], [`answer()`]).

use bevy::picking::Pickable;
use bevy::picking::events::{PointerClick, PointerOut, PointerOver};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::text::{FontSize, LineHeight};
use bevy_kira_audio::prelude::{Audio, AudioChannel, AudioControl};
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
use crate::npcs::{Corpse, Dies, Npcs, ScriptName, Still, Talk, Talks, Trades, Walk};
use crate::scripts::{self, SHOWN};
use crate::sfx::Sfx;
use crate::voices;

/// Within how many times the two creatures' reach the talk opens at once (`0x529C60`).
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
/// The dialogue whose function shows a townsperson's generic line, its first text replaced by the
/// line (`0x549920`: the talk 54,321 of the add-on's trees).
const GENERIC: &str = "NPC_UW_DLG";
/// The record of the dice of a creature's generic line, by which its own seed makes them, as the
/// original seeds its `rand()` with the creature's id at each query.
const GENERIC_ROLL: usize = 0x5499;

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
/// As far for the buttons' labels in their row: measured on the maintainer's frames of the
/// original at 3,840 × 2,160 (#2419), the labels' tops 1,375 pixels below the window's top edge
/// where the texts' drop put them 3 lower.
const BUTTON_DROP: f32 = 3.0;
/// The room below a row of buttons past its line and 4 (`0x6E7AF0`): measured on the
/// maintainer's frames of the original at 3,840 × 2,160 (#2419), whose window ends 2 of the
/// interface's units lower below its buttons than lines of 25 put it, its top and its labels
/// where the game's stand (**hypothesis**: the row takes the line of the buttons' font, which
/// the client asks apart from the texts'). Without it the labels' feet reached the frame's edge.
const BUTTON_FOOT: f32 = 2.0;
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
/// The modes of an `InfoPlayer` that show its text (`0x4AE350`): 0 and 1, 2 with the music
/// `0x1B61` once, which has no file, and [`CLICKS`].
const SHOWS: [i32; 4] = [0, 1, 2, 3];
/// The mode of an `InfoPlayer` that plays `UI_CLICK01` (7000) in the middle with its text or
/// without, as the add-on's side quest 7618 asks for (#2891).
const CLICKS: i32 = 3;

/// What a talk opens (`0x52AB70`): the creature's dialogue by its function's name, or, for a
/// trader without one (its number below 1 or above 200,000), a smith's, a merchant's or a combat
/// master's window.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Speech {
    Dialogue(String),
    /// A townsperson's generic line of the Underworld, in the window of [`GENERIC`] (#2887).
    Line(String),
    Smith,
    Merchant,
    Master,
    /// The horse window of a stable's horse or a horse trader (`0x529930`, #2300).
    Horses,
}

impl std::fmt::Display for Speech {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dialogue(dialogue) => f.write_str(dialogue),
            Self::Line(line) => write!(f, "the generic line {line}"),
            Self::Smith => f.write_str("the smith's window"),
            Self::Merchant => f.write_str("the merchant's window"),
            Self::Horses => f.write_str("the horse window"),
            Self::Master => f.write_str("the combat master's window"),
        }
    }
}

/// What the creature of `talk`, `mind` and `trades` opens as the hero speaks to it, of `npcs`:
/// its dialogue ([`dialogue_of`]), else a townsperson's generic `line` where the Underworld gives
/// it one ([`Lines::line`]), else the smith's window where it is a smith (`+0x200` bit
/// `0x1000`, `quests::SMITH`, #1662), else the merchant's window where it is a merchant (bit
/// `0x2000`, `quests::MERCHANT`, #1618), else the master's window where it is a combat master
/// (bit `0x4000`, `quests::WIZARD`); a horse trader's talk stamp (`quests::TRADER`, 180,000)
/// opens the horse window before any dialogue (`0x52BA16`, #2300).
pub(crate) fn speech_of(
    (talk, mind, trades): (Option<&Talk>, &Mind, Option<&Trades>),
    npcs: Option<&Npcs>,
    line: impl FnOnce() -> Option<String>,
) -> Option<Speech> {
    if trades.is_some_and(|trades| trades.0 & opensacred_rules::quests::TRADER != 0) {
        return Some(Speech::Horses);
    }
    dialogue_of(talk, mind, npcs)
        .map(Speech::Dialogue)
        .or_else(|| generic(talk, mind).then(line).flatten().map(Speech::Line))
        .or_else(|| {
            let trades = trades.map_or(0, |trades| trades.0);
            if trades & opensacred_rules::quests::SMITH != 0 {
                Some(Speech::Smith)
            } else if trades & opensacred_rules::quests::MERCHANT != 0 {
                Some(Speech::Merchant)
            } else if trades & opensacred_rules::quests::WIZARD != 0 {
                Some(Speech::Master)
            } else {
                None
            }
        })
}

/// The hero speaking to `creature`, of `dialogue`: going to it until the talk opens.
#[derive(Component, Clone, Debug)]
pub struct Talking {
    creature: Entity,
    dialogue: Speech,
    open: bool,
    /// Whether the hero walks to the creature until within their reach, having stood farther than
    /// 1.5 times it as the talk began; unknown until then.
    walks: Option<bool>,
    /// How many times the hero has stopped short of the creature.
    stops: u8,
}

/// The hero's walk to talk as a save keeps it (#3763): the creature by its name and what it is
/// to say; a talk whose window is open is no save's, which the window refuses (`saves::allowed`).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedTalking {
    #[serde(with = "crate::names::required")]
    creature: Entity,
    dialogue: Speech,
    walks: Option<bool>,
    stops: u8,
}

impl bevy::ecs::entity::MapEntities for SavedTalking {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.creature = mapper.get_mapped(self.creature);
    }
}

impl crate::doings::Saves for Talking {
    type Saved = SavedTalking;

    fn saved(&self, _: &crate::doings::Moment) -> Option<SavedTalking> {
        (!self.open).then(|| SavedTalking {
            creature: self.creature,
            dialogue: self.dialogue.clone(),
            walks: self.walks,
            stops: self.stops,
        })
    }

    fn again(saved: SavedTalking, _: &crate::doings::Moment) -> Option<Self> {
        Some(Self {
            creature: crate::names::found(Some(saved.creature))?,
            dialogue: saved.dialogue,
            open: false,
            walks: saved.walks,
            stops: saved.stops,
        })
    }
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

/// A text's runs of words, each with its colour ([`painted`]).
type Runs = [(String, Color)];

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

#[cfg(test)]
impl Shown {
    /// A window of no buttons and no text with the creature `creature`, for the tests of what
    /// its showing holds back.
    pub(crate) fn of(creature: Entity) -> Self {
        Shown {
            root: Entity::PLACEHOLDER,
            creature,
            functions: Vec::new(),
            text: String::new(),
            labels: Vec::new(),
        }
    }
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
    townsfolk(mind).then(|| region(mind.creature)).flatten()
}

/// Whether the creature of `mind` is of the townsfolk's sides and flags and hostile neither way,
/// for whom a type's dialogue and a generic line speak (`0x549920`).
fn townsfolk(mind: &Mind) -> bool {
    REGION_SIDES.contains(&mind.side)
        && mind.flags & REGION_FLAGS != 0
        && mind.flags & NOT_REGION == 0
        && !mind.hostile()
}

/// Whether the creature of `talk` and `mind` may say a generic line: a townsperson of no dialogue
/// of its own (`0x549920`).
pub(crate) fn generic(talk: Option<&Talk>, mind: &Mind) -> bool {
    talk.is_none() && townsfolk(mind)
}

/// The regions' atmospheres, of the scripts' variables `atmo_rg<n>` (the interpreter's
/// `+0x7750`, which `IncVar`, `DecVar`, `SetVar` and `RndVar` of such a variable keep,
/// `0x49CAB8`), by which the generic lines go (#2887); copied from the scripts at each frame. No
/// tree names such a variable (the add-on's keep theirs in `atmos_rg<n>`), so they stand at 0, as
/// the original's.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub(crate) struct Atmospheres(Vec<i32>);

impl Atmospheres {
    /// The atmosphere of the region `region`.
    fn of(&self, region: u16) -> i32 {
        self.0.get(usize::from(region)).copied().unwrap_or(0)
    }
}

/// Copies the regions' atmospheres from the scripts' variables.
fn atmospheres(live: Option<NonSend<scripts::Live>>, mut atmospheres: ResMut<Atmospheres>) {
    let Some(live) = live else {
        return;
    };
    let values = (0..=*opensacred_rules::quests::UNDERWORLD.end())
        .map(|region| live.variable(&format!("atmo_rg{region}")))
        .collect();
    atmospheres.set_if_neq(Atmospheres(values));
}

/// The generic line that a townsperson of the kind `kind` standing at `at` says, its dice seeded
/// by `seed` (`opensacred_rules::quests::underworld_line`): the region of its sector's area
/// (`0x602290`), the regions' atmospheres.
pub(crate) fn generic_line(
    (kind, at, seed): (u32, Vec3, u32),
    streamer: &crate::sectors::Streamer,
    atmospheres: &Atmospheres,
) -> Option<String> {
    let sector = crate::music::sector_of(at.x, at.z);
    let region = opensacred_rules::quests::region(streamer.area(sector)?);
    let mut dice = opensacred_rules::spawning::Rng::of(seed, 0, GENERIC_ROLL);
    opensacred_rules::quests::underworld_line(
        kind,
        region,
        |region| atmospheres.of(region),
        &mut dice,
    )
}

/// What the generic lines of the creatures take: the sectors' areas, the regions' atmospheres,
/// and the creatures' places and placements, by which their dice are seeded.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Lines<'w, 's> {
    streamer: Option<Res<'w, crate::sectors::Streamer>>,
    atmospheres: Res<'w, Atmospheres>,
    places: Query<
        'w,
        's,
        (
            &'static GlobalTransform,
            Option<&'static crate::npcs::PlacementIndex>,
        ),
    >,
}

impl Lines<'_, '_> {
    /// The generic line that the creature `entity` of `mind` says ([`generic_line`]), its dice
    /// seeded by its placement, else its entity, so that it says the same while the atmospheres
    /// stand, as the original's seeded by its id.
    pub(crate) fn line(&self, entity: Entity, mind: &Mind) -> Option<String> {
        let (at, placement) = self.places.get(entity).ok()?;
        let seed = placement.map_or(entity.index_u32(), |placement| {
            u32::try_from(placement.0).unwrap_or(u32::MAX)
        });
        generic_line(
            (mind.creature, at.translation(), seed),
            self.streamer.as_deref()?,
            &self.atmospheres,
        )
    }
}

/// On a press of the left button, the creature of a dialogue that the cursor picks
/// (`under_cursor`) becomes the hero's to speak to, the hero walking to it (`0x53A170`): one
/// alive, or a dead one whose body its dialogue holds ([`corpse_speech`]); any other press ends a
/// talk not open yet.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their \
              filters"
)]
pub(crate) fn pick(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, under): (
        Res<crate::hud::Pointer>,
        Res<crate::under_cursor::UnderCursor>,
    ),
    mut aim: ResMut<Aim>,
    (npcs, lines): (Option<Res<Npcs>>, Lines),
    creatures: Query<
        (
            Entity,
            &GlobalTransform,
            &Mind,
            &Health,
            Option<&Talk>,
            Option<&Trades>,
            (&crate::crowd::InTheWay, Has<crate::stables::Stabled>),
        ),
        Without<Dies>,
    >,
    corpses: Query<(Entity, &GlobalTransform, &Talk, &Corpse), With<Dies>>,
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
    if pointer.0.is_none() {
        return;
    }
    let under = under.0;
    let living = under.and_then(|entity| {
        let (_, at, mind, health, talk, trades, (way, stabled)) = creatures.get(entity).ok()?;
        if health.current == 0 {
            return None;
        }
        // A stable's horse's talk stamp opens the horse window (#2300).
        let dialogue = if stabled {
            Speech::Horses
        } else {
            speech_of((talk, mind, trades), npcs.as_deref(), || {
                lines.line(entity, mind)
            })?
        };
        Some((entity, (at.translation(), way.level), dialogue))
    });
    let dead = || {
        let (entity, at, talk, corpse) = corpses.get(under?).ok()?;
        Some((
            entity,
            (at.translation(), corpse.level),
            corpse_speech(talk),
        ))
    };
    let Some((creature, (at, level), dialogue)) = living.or_else(dead) else {
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
                walks: None,
                stops: 0,
            });
    }
}

/// What a dead creature of the dialogue `talk` opens as the hero speaks to it: its dialogue. Its
/// body lies on while it holds one (the dying action's hold, `0x53ECA0`'s lying step, of a
/// dialogue `+0x245` from 1 to 59,999), a click on it sends the hero to it as to a living one's
/// (`0x6178B2`, the order 7), and a script's dialogue given at its death speaks there: Wilbur's of
/// quest 45, `DlgWillbur45`, which starts quest 51 (#2444).
fn corpse_speech(talk: &Talk) -> Speech {
    Speech::Dialogue(talk.0.clone())
}

/// Whether the talk opens with the hero `distance` from the creature, `reach` their reach, as the
/// first step of the hero's AI's state 10 decides (`0x529C60`): at once where the hero stood
/// within 1.5 times the reach as it began (not `walks`); else once it comes within the reach, or
/// once its walk to the creature has `ended` within 1.5 times the reach.
fn opens(distance: f32, reach: f32, walks: bool, ended: bool) -> bool {
    !walks || distance <= reach || (ended && distance <= OPENS_WITHIN * reach)
}

/// The hero going to speak to a creature: within 1.5 times their reach it stops at once, farther
/// it walks to the creature until within the reach, heading for it again as it moves ([`opens`]);
/// then the creature stops, both turn to each other and the creature plays its talk, and the
/// dialogue runs ([`open`]). The hero gives the talk up where it stops short of it again and
/// again, or where the creature is gone or dead, but a dead one whose dialogue holds its body,
/// which neither turns nor plays a talk (`0x529C60`, `0x52AB70`; [`corpse_speech`]).
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
    corpses: Query<(&Transform, &Corpse), (With<Talk>, With<Dies>, Without<Body>)>,
) {
    for (hero, mut transform, mut body, mut talking, size) in &mut heroes {
        if talking.open {
            continue;
        }
        let size = size.map_or(minds::SIZE, |size| size.0);
        let (mut theirs, reach, level) = match creatures.get_mut(talking.creature) {
            Ok((theirs, mind, health, way)) if health.current > 0 => {
                (Some(theirs), minds::reach_f32(size, mind.size()), way.level)
            }
            Ok(_) => {
                commands.entity(hero).remove::<Talking>();
                continue;
            }
            Err(_) => match corpses.get(talking.creature) {
                Ok((_, corpse)) => (
                    None,
                    minds::reach_f32(size, minds::size(corpse.creature, false)),
                    corpse.level,
                ),
                Err(_) => {
                    commands.entity(hero).remove::<Talking>();
                    continue;
                }
            },
        };
        let Some(there) = theirs
            .as_ref()
            .map(|theirs| theirs.translation)
            .or_else(|| {
                corpses
                    .get(talking.creature)
                    .ok()
                    .map(|(lying, _)| lying.translation)
            })
        else {
            continue;
        };
        let here = transform.translation;
        let distance = (there - here).xz().length();
        let walks = *talking.walks.get_or_insert(distance > OPENS_WITHIN * reach);
        // An order waits for the world's next tick (`hero::take_orders`).
        let ended = !body.ordered() && body.target().is_none();
        if !opens(distance, reach, walks, ended) {
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
                body.send_onto(there, level);
            }
            continue;
        }
        body.stop();
        transform.rotation = hero::facing(there - here);
        talking.open = true;
        let creature = talking.creature;
        // A living one stops, turns to the hero and plays its talk; a body lies as it fell.
        if let Some(theirs) = theirs.as_mut() {
            theirs.rotation = hero::facing(here - there);
            commands
                .entity(creature)
                .remove::<Walk>()
                .insert((Still, Talks));
        }
        match talking.dialogue.clone() {
            Speech::Dialogue(dialogue) => {
                commands.queue(move |world: &mut World| {
                    open(world, (hero, creature), &dialogue, None);
                });
            }
            Speech::Line(line) => {
                commands.queue(move |world: &mut World| {
                    open(world, (hero, creature), GENERIC, Some(&line));
                });
            }
            // A smith, a merchant or a master shows its window beside the inventory, and the
            // talk ends at once (`0x52AB70`).
            Speech::Smith => {
                commands.queue(move |world: &mut World| {
                    crate::blacksmith::open(world, creature);
                    end(world, hero, creature);
                });
            }
            Speech::Merchant => {
                commands.queue(move |world: &mut World| {
                    crate::merchant::open(world, creature);
                    end(world, hero, creature);
                });
            }
            Speech::Master => {
                commands.queue(move |world: &mut World| {
                    crate::master::open(world, creature);
                    end(world, hero, creature);
                });
            }
            Speech::Horses => {
                commands.queue(move |world: &mut World| {
                    crate::stables::open(world, creature);
                    end(world, hero, creature);
                });
            }
        }
    }
}

/// Runs the creature's dialogue (`scripts::speak`) and shows its window: its name, its texts and
/// its first four buttons, its sounds played, its first text a generic `line` where one is given
/// (`0x549920`'s line in place of `Dialog:NPC_UW_DLG`'s, #2887); a dialogue that takes the
/// creature's away (`DelDlg`) takes it; one without a text ends the talk at once.
fn open(world: &mut World, (hero, creature): (Entity, Entity), dialogue: &str, line: Option<&str>) {
    // The hero's own talk's picture, of its dialogue's entry as it stands before the talk's
    // function runs, whose own `SetIcon`s change it for the next talk (`0x499E90`, #2418).
    let picture = (creature == hero)
        .then(|| {
            let icon = world.get_resource::<Npcs>()?.icon(dialogue)?;
            let class = world.get_resource::<hero::Hero>()?.creature;
            tip_image(icon, class)
        })
        .flatten();
    let mut window = scripts::speak(world, dialogue).unwrap_or_default();
    if let Some(line) = line {
        match window.texts.first_mut() {
            Some(first) => *first = line.to_owned(),
            None => window.texts.push(line.to_owned()),
        }
    }
    if window.untalks
        && let Ok(mut entity) = world.get_entity_mut(creature)
    {
        entity.remove::<Talk>();
        if let Some(mut mind) = entity.get_mut::<Mind>() {
            mind.dialogue = false;
        }
    }
    let texts = world.get_resource::<Texts>();
    let raw: Vec<&str> = window
        .texts
        .iter()
        .filter_map(|key| texts.and_then(|texts| texts.named(key)))
        .collect();
    let lines: Vec<String> = raw.iter().map(|text| shown_text(text)).collect();
    // The texts as their colours' tags paint them, a line between them (#2420).
    let mut runs: Vec<(String, Color)> = Vec::new();
    for (index, text) in raw.iter().enumerate() {
        if index > 0 {
            runs.push(("\n".to_owned(), WORDS));
        }
        runs.extend(painted(text));
    }
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
    voice(world, &window.sounds, creature);
    let font: Handle<Font> = world.resource::<AssetServer>().load(FONT);
    // Its portrait by its type (`0x4345C0`, #1032); the hero's own talk its dialogue's picture
    // where it has one (`0x6E8230`, #2418).
    let portrait = match picture {
        Some(index) => world
            .get_resource::<UiImages>()
            .and_then(|images| images.pixels(index))
            .map(|(file, rect)| (file.to_owned(), rect)),
        None => world
            .get::<Mind>(creature)
            .map(|mind| mind.creature)
            .or_else(|| world.get::<Corpse>(creature).map(|corpse| corpse.creature))
            .and_then(|kind| {
                let (file, rect) = world.get_resource::<Portraits>()?.of(kind)?;
                Some((file.to_owned(), rect))
            }),
    }
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
        (&runs, &buttons),
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

/// A text of `global.res` as the window paints it (#2420): its runs, each a stretch of
/// [`shown_text`]'s words and its colour, white at first; `<color=AARRGGBB>` the words after it
/// until another colour's tag, across the line breaks (seen: the new game's first window, its
/// title white and every paragraph after the tag `0xFFFFFF80` in the original), `<colorl=…>` the
/// rest of its line and `<colorw=…>` the word after it (the tags' three kinds, `0x673AD0`). The
/// texts reset a colour with a tag of white (`<color=FFFFFFFF>` before a key's name, as the
/// bar's), which a colour of a line would not need.
pub(crate) fn painted(text: &str) -> Vec<(String, Color)> {
    let mut runs: Vec<(String, Color)> = Vec::new();
    let mut push = |words: &str, colour: Color| {
        if words.is_empty() {
            return;
        }
        match runs.last_mut() {
            Some((last, was)) if *was == colour => last.push_str(words),
            _ => runs.push((words.to_owned(), colour)),
        }
    };
    let (mut held, mut line, mut word) = (WORDS, None::<Color>, None::<Color>);
    let mut rest = text;
    loop {
        let open = rest.find('<').unwrap_or(rest.len());
        let mut words = &rest[..open];
        if let Some(colour) = word.take() {
            let end = words.find(char::is_whitespace).unwrap_or(words.len());
            push(&words[..end], colour);
            words = &words[end..];
        }
        push(words, line.unwrap_or(held));
        if open == rest.len() {
            break;
        }
        let Some(close) = rest[open..].find('>') else {
            push(&rest[open..], line.unwrap_or(held));
            break;
        };
        let tag = &rest[open + 1..open + close];
        let colour = |prefix: &str| {
            tag.strip_prefix(prefix)
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .map(crate::tooltip::argb)
        };
        if tag == "n" {
            push("\n", line.unwrap_or(held));
            line = None;
        } else if let Some(colour) = colour("colorw=") {
            word = Some(colour);
        } else if let Some(colour) = colour("colorl=") {
            line = Some(colour);
        } else if let Some(colour) = colour("color=") {
            held = colour;
            line = None;
        } else if !tag.starts_with("color") {
            // A speaker's sigh or groan, kept as it stands (`shown_text`).
            push(&rest[open..=open + close], line.unwrap_or(held));
        }
        rest = &rest[open + close + 1..];
    }
    runs
}

/// The key of a name of the scripts' resources, `res:<key>` (case aside), as the window looks
/// its string up: `res:17095` the string of `17095`.
pub(crate) fn resource(name: &str) -> &str {
    match name.get(..4) {
        Some(prefix) if prefix.eq_ignore_ascii_case("res:") => &name[4..],
        _ => name,
    }
}

/// Plays the dialogue's sounds of the creature `speaker`, its voice-overs under their texts' keys
/// (`sound.pak`'s `SOUND_FX_<key>`, `0x4A9730`), each the line by name that stops the one before,
/// the hero's own too (`0x693FE0`, `voices::NamedLine`); one whose name holds `_SFX` an effect
/// at the speaker ([`effect_at`]).
fn voice(world: &mut World, sounds: &[String], speaker: Entity) {
    for sound in sounds {
        if effect_line(sound) {
            effect_at(world, sound, Some(speaker));
            continue;
        }
        let assets = world.resource::<AssetServer>().clone();
        let Some(mut sfx) = world.get_resource_mut::<Sfx>() else {
            return;
        };
        let Some(sample) = line_sample(&mut sfx, sound, &assets) else {
            continue;
        };
        let Some(speech) = world.get_resource::<AudioChannel<crate::volumes::Speech>>() else {
            return;
        };
        let instance = speech.play(sample).handle();
        voices::name_line(world, Some(instance));
    }
}

/// Plays the line by name `sound` whose name holds `_SFX` as the original's sound manager does
/// (`0x693FE0` at `0x694446`): placed at its `speaker` where it has one, else in the middle, at
/// the effects' volume, and not the line by name, so that it stops no line and none stops it.
fn effect_at(world: &mut World, sound: &str, speaker: Option<Entity>) {
    let Some((id, _)) = world
        .get_resource::<Sfx>()
        .and_then(|sfx| sfx.named(&sound_name(sound)))
    else {
        debug!("talk: no sound {}", sound_name(sound));
        return;
    };
    world.write_message(crate::sfx::Play {
        sound: crate::sfx::Of::Id(id),
        at: speaker,
        rate: (1.0, crate::sfx::Roll::None),
    });
}

/// Whether the line by name `sound` is an effect, its name past `SOUND_FX_` holding `_SFX`
/// (`0x9DF24C`).
fn effect_line(sound: &str) -> bool {
    sound_name(sound)
        .get("SOUND_FX_".len()..)
        .is_some_and(|rest| rest.contains("_SFX"))
}

/// The name of the sound that a `PlaySound` names `sound` (`0x4A9730`): `SOUND_FX_` put before a
/// name that does not begin with `SOUND_FX` (its first eight letters, case aside), as a
/// dialogue's voice-over names its text's key and an announcement its sound's whole name.
pub(crate) fn sound_name(sound: &str) -> String {
    let sound = sound.to_ascii_uppercase();
    if sound.starts_with("SOUND_FX") {
        sound
    } else {
        format!("SOUND_FX_{sound}")
    }
}

/// The sample of the sound that a `PlaySound` names `sound` ([`sound_name`]), none for a name
/// that no sound has (`0x676170`).
fn line_sample(
    sfx: &mut Sfx,
    sound: &str,
    assets: &AssetServer,
) -> Option<Handle<bevy_kira_audio::AudioSource>> {
    let name = sound_name(sound);
    let Some((id, file)) = sfx.named(&name) else {
        debug!("talk: no sound {name}");
        return None;
    };
    Some(sfx.sample(id, file, assets))
}

/// Plays the scripts' sounds of a `PlaySound` outside a dialogue's window, each the line by name
/// that stops the one before (`0x693FE0`; #1826), as a window's voice-over plays.
fn lines(
    mut lines: MessageReader<scripts::ScriptLine>,
    (assets, audio): (Res<AssetServer>, Option<Res<Audio>>),
    speech: Option<Res<AudioChannel<crate::volumes::Speech>>>,
    mut sfx: Option<ResMut<Sfx>>,
    mut named: Option<ResMut<voices::NamedLine>>,
    mut instances: Option<ResMut<Assets<bevy_kira_audio::AudioInstance>>>,
) {
    let (Some(sfx), Some(audio), Some(speech), Some(named), Some(instances)) = (
        sfx.as_deref_mut(),
        audio,
        speech,
        named.as_deref_mut(),
        instances.as_deref_mut(),
    ) else {
        lines.clear();
        return;
    };
    for scripts::ScriptLine(sound) in lines.read() {
        let Some(sample) = line_sample(sfx, sound, &assets) else {
            continue;
        };
        info!("talk: the scripts play {sound}");
        // An `_SFX` name, which names no speaker here, plays in the middle at the effects'
        // volume and is not the line by name (`0x694446`).
        if effect_line(sound) {
            audio.play(sample);
            continue;
        }
        let instance = speech.play(sample).handle();
        named.name(instances, Some(instance), false);
    }
}

/// Stops the line by name that plays, the window's voice-over or the hero's own line
/// (`0x693FB0`).
fn hush(world: &mut World) {
    voices::name_line(world, None);
}

/// The interface's image that the hero's own talk shows for its dialogue's picture `icon` (the
/// entry's `+0x48`, which `SetIcon` writes), the hero of the creature type `class`, as the
/// window's `0x6E8230` maps it: the tutorial's and the scripts' pictures `0x400` to `0x42A` by a
/// table of the client, `0x457` the hero's class's portrait; none for any other (#2418).
fn tip_image(icon: i32, class: u32) -> Option<u32> {
    let icon = u32::try_from(icon).ok()?;
    Some(match icon {
        0x400 => 0x411,
        0x401 => 0x412,
        0x402 => 0x417,
        0x403..=0x412 => 0x447 + (icon - 0x403),
        0x413 => 0x419,
        0x414..=0x423 => 0x46B + (icon - 0x414),
        0x424 => 0x686,
        0x425 => 0x662,
        0x426 => 0x45F,
        0x427..=0x42A => 0x6D6 + (icon - 0x427),
        0x457 => match class {
            1 => 0x3C4,
            2 => 0x3C5,
            3 => 0x3C3,
            4 => 0x34E,
            5 => 0x383,
            6 | 7 => 0x3C8,
            8 => 0x6C3,
            9 => 0x6C2,
            _ => return None,
        },
        _ => return None,
    })
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
    (text, buttons): (&Runs, &[(String, String)]),
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
                            block
                                .spawn((
                                    Text::default(),
                                    words.clone(),
                                    TextColor(WORDS),
                                    TextLayout::justify(Justify::Justified),
                                ))
                                .with_children(|lines| {
                                    for (run, colour) in text {
                                        lines.spawn((
                                            TextSpan::new(run.clone()),
                                            words.clone(),
                                            TextColor(*colour),
                                        ));
                                    }
                                });
                        });
                    if !buttons.is_empty() {
                        panel
                            .spawn(Node {
                                height: Val::Px(LINE + FOOT + BUTTON_FOOT),
                                padding: UiRect {
                                    top: Val::Px(BUTTON_DROP),
                                    bottom: Val::Px(BUTTON_FOOT),
                                    ..default()
                                },
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
/// window once the hero and the creature stand farther than 1,500 apart or the creature is gone,
/// its body lying on (`0x5370B0`): a dead one's dialogue that takes its dialogue away (`DelDlg`)
/// keeps its window to the end ([`corpse_speech`]). The press closes it only while the game
/// runs, as the original's talk tests it at its ticks: a press on the menu over a game or on the
/// pause's window, which show over the dialogue's (#2877), leaves it.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn dismiss(
    mut commands: Commands,
    shown: Option<Res<Shown>>,
    keys: crate::controls::Keys,
    (buttons, play): (
        Res<ButtonInput<MouseButton>>,
        Option<Res<State<crate::pause::Play>>>,
    ),
    heroes: Query<&Transform, With<Body>>,
    creatures: Query<&Transform, Or<(Without<Dies>, With<Corpse>)>>,
) {
    let Some(shown) = shown else {
        return;
    };
    let running = play.is_none_or(|play| *play.get() == crate::pause::Play::Running);
    let pressed = running
        && shown.functions.is_empty()
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

/// Hides the window while the menu over a game shows, and shows it again as the menu goes: the
/// original draws its menu alone over a talk, and the window again once the menu is closed,
/// where its pause's window darkens the dialogue's with the rest of the screen (seen, #2877).
/// The pauses that show no menu leave it: the scripts' own, under which a tip's window waits,
/// and the key P's.
fn veil(
    shown: Option<Res<Shown>>,
    play: Option<Res<State<crate::pause::Play>>>,
    (still, busy): (
        Option<Res<crate::pause::Still>>,
        Option<Res<crate::pause::Busy>>,
    ),
    mut windows: Query<&mut Visibility>,
) {
    let Some(shown) = shown else {
        return;
    };
    let menu = play.is_some_and(|play| *play.get() == crate::pause::Play::Paused)
        && !still.is_some_and(|still| still.0)
        && !busy.is_some_and(|busy| busy.0);
    if let Ok(mut visibility) = windows.get_mut(shown.root) {
        visibility.set_if_neq(if menu {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
}

/// Shows the scripts' `InfoPlayer`s' texts (#1033): of a mode that shows one, each, as lines
/// under those that show (`info_lines`, #2499), the mode 3 with its click (#2891); of the other
/// modes `fades`, `movies`, `pictures` and `statistics` take theirs before it, and the rest (an
/// event, the interface's states) are not followed.
pub(crate) fn notices(
    mut infos: ResMut<scripts::Infos>,
    texts: Option<Res<Texts>>,
    mut shown: ResMut<crate::info_lines::Shown>,
    mut sounds: MessageWriter<crate::sfx::Play>,
) {
    for (mode, key) in infos.0.drain(..) {
        if !SHOWS.contains(&mode) {
            debug!("talk: an InfoPlayer of the mode {mode} ({key}) is not followed");
            continue;
        }
        if mode == CLICKS {
            crate::inventory::play(&mut sounds, crate::inventory::CLICK);
        }
        // A count's line, made of its parts (`scripts::count_line`, #3448).
        if let Some(line) = texts
            .as_deref()
            .and_then(|texts| scripts::count_line(&key, texts))
        {
            info!("talk: the player is shown {line}");
            shown.0.push(line);
            continue;
        }
        match texts
            .as_deref()
            .and_then(|texts| texts.named(resource(&key)))
        {
            Some(text) => {
                let text = shown_text(text);
                info!(
                    "talk: the player is shown {}",
                    text.lines().next().unwrap_or_default()
                );
                shown.0.push(text);
            }
            None => debug!("talk: no text {key}"),
        }
    }
}

/// Begins the talks that the scripts ask for (`scripts::Popups`), one at a time, once no window
/// shows: the hero's own of a dialogue that a `Popup` gives it (`0x4A7760`, `0x54CBB0`; #1033),
/// its window without a name, and the hero's with the creature that an `NPC_TalkTo` gives a
/// dialogue (`0x49DCF0`, #1037), as a click on it begins one.
pub(crate) fn popups(
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
            commands.queue(move |world: &mut World| open(world, (hero, hero), &dialogue, None));
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

/// Sends the hero to speak to `creature`, or to the nearest creature that a talk opens something
/// for ([`Speech`]: a dialogue, a dead one's that holds its body among them, a trader's window or
/// the horse window; the command `talk`), as a click on it does: a line of which.
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
            (&crate::crowd::InTheWay, Has<crate::stables::Stabled>),
        ), Without<Dies>>();
        let npcs = world.get_resource::<Npcs>();
        creatures
            .iter(world)
            .filter(|(entity, _, _, health, ..)| {
                health.current > 0 && creature.is_none_or(|c| c == *entity)
            })
            .filter_map(|(entity, at, mind, _, (talk, trades), (way, stabled))| {
                let speech = if stabled {
                    Speech::Horses
                } else {
                    speech_of((talk, mind, trades), npcs, || {
                        let placement = world.get::<crate::npcs::PlacementIndex>(entity);
                        let seed = placement.map_or(entity.index_u32(), |placement| {
                            u32::try_from(placement.0).unwrap_or(u32::MAX)
                        });
                        generic_line(
                            (mind.creature, at.translation, seed),
                            world.get_resource::<crate::sectors::Streamer>()?,
                            world.get_resource::<Atmospheres>()?,
                        )
                    })?
                };
                Some((entity, at.translation, way.level, speech))
            })
            .collect()
    };
    // A dead one whose dialogue holds its body (#2444).
    let mut corpses = world.query_filtered::<(Entity, &Transform, &Talk, &Corpse), With<Dies>>();
    let found: Vec<_> = found
        .into_iter()
        .chain(
            corpses
                .iter(world)
                .filter(|(entity, ..)| creature.is_none_or(|c| c == *entity))
                .map(|(entity, at, talk, corpse)| {
                    (entity, at.translation, corpse.level, corpse_speech(talk))
                }),
        )
        .collect();
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
            walks: None,
            stops: 0,
        });
    Ok(format!("the hero goes to speak to {creature} ({dialogue})"))
}

/// The new game's first windows that the command `new-game` and a game that the command line
/// names close, unless `--leave-open` is given, as the original's driver does (#1343): the
/// start's `Pre_Tutorial`, which every hero's start script's `Popup` gives, by its OK
/// (`Tutorial_Trigger`), then the tutorial's first tip by its No more Tips (`TT_KEINEMEHR`),
/// after which no tip comes.
#[derive(Resource, Default)]
pub struct CloseFirst;

/// The functions of the first window's OK and of a tip's No more Tips.
const FIRST_OK: &str = "Tutorial_Trigger";
const NO_MORE_TIPS: &str = "TT_KEINEMEHR";

/// Closes the first windows of a new game that asks for it ([`CloseFirst`]): the window
/// whose button runs [`FIRST_OK`] by it, then the first whose button runs [`NO_MORE_TIPS`] by
/// it, which ends the closing.
fn close_first(world: &mut World) {
    if !world.contains_resource::<CloseFirst>() {
        return;
    }
    let Some(shown) = world.get_resource::<Shown>() else {
        return;
    };
    let position = |name: &str| shown.functions.iter().position(|function| function == name);
    let (first, tips) = (position(FIRST_OK), position(NO_MORE_TIPS));
    match (first, tips) {
        (Some(ok), _) => {
            info!("talk: the first window closed by its OK");
            choose(world, ok);
        }
        (None, Some(no_more)) => {
            info!("talk: the tutorial's tips closed by No more Tips");
            world.remove_resource::<CloseFirst>();
            choose(world, no_more);
        }
        (None, None) => {}
    }
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
        app.hold_clicks(no_window)
            .init_resource::<Atmospheres>()
            .add_systems(OnExit(AppState::InGame), forget)
            .add_systems(Update, atmospheres.in_set(Playing))
            .add_systems(
                Update,
                (
                    pick.in_set(Control)
                        .in_set(crate::frame::OnWorld)
                        .after(attack::aim)
                        .after(hero::steer)
                        .after(crate::doors::pick)
                        .before(crate::pickup::pick),
                    approach.after(hero::run),
                )
                    .in_set(Unpaused)
                    .in_set(Step::HeroMove),
            )
            .add_systems(
                Update,
                (
                    dismiss,
                    veil,
                    notices.after(popups),
                    popups,
                    close_first.after(popups),
                )
                    .in_set(Playing),
            )
            .add_systems(Update, lines.after(Step::Npcs).in_set(Playing));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_infoplayer_of_the_mode_3_clicks_and_the_others_do_not() {
        let mut app = App::new();
        app.init_resource::<scripts::Infos>()
            .init_resource::<crate::info_lines::Shown>()
            .add_message::<crate::sfx::Play>()
            .add_systems(Update, notices);
        app.world_mut().resource_mut::<scripts::Infos>().0 = vec![
            (0, "res:17643".into()),
            (CLICKS, "res:NQ_7618_INFOPLAYER".into()),
            (2, "res:17643".into()),
            (9, "res:17643".into()),
        ];
        app.update();
        let played: Vec<u32> = app
            .world_mut()
            .resource_mut::<Messages<crate::sfx::Play>>()
            .drain()
            .filter_map(|play| match play.sound {
                crate::sfx::Of::Id(id) => Some(id),
                crate::sfx::Of::Profile(..) => None,
            })
            .collect();
        assert_eq!(played, [crate::inventory::CLICK]);
        assert!(app.world().resource::<scripts::Infos>().0.is_empty());
    }

    #[test]
    fn a_dead_creature_whose_dialogue_holds_its_body_is_spoken_to() {
        let mut world = World::new();
        let hero = world.spawn((Body::standing(), Transform::default())).id();
        let corpse = |world: &mut World, talk: Option<&str>| {
            let mut entity = world.spawn((
                Transform::from_xyz(100.0, 0.0, 0.0),
                Dies::new(0),
                Corpse {
                    creature: 83,
                    class: 0,
                    dialogue: talk.is_some(),
                    level: None,
                    hostile: false,
                    flags: 0,
                },
            ));
            if let Some(talk) = talk {
                entity.insert(Talk(talk.to_owned()));
            }
            entity.id()
        };
        // A body without a dialogue: nobody to speak to.
        corpse(&mut world, None);
        assert!(talk_to(&mut world, None).is_err());
        // Wilbur's, given his dialogue as he lies (#2444): the hero goes to speak to him.
        let wilbur = corpse(&mut world, Some("DlgWillbur45"));
        let said = talk_to(&mut world, None).unwrap();
        assert!(said.contains("DlgWillbur45"), "{said}");
        let talking = world.get::<Talking>(hero).unwrap();
        assert_eq!(talking.creature, wilbur);
        assert!(matches!(&talking.dialogue, Speech::Dialogue(d) if d == "DlgWillbur45"));
    }

    #[test]
    fn under_a_pause_a_press_leaves_the_window_and_the_menu_over_a_game_hides_it() {
        use crate::pause::{Busy, Play, Still};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .insert_state(AppState::InGame)
            .add_sub_state::<Play>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<Still>()
            .init_resource::<Busy>()
            .add_systems(Update, (dismiss, veil));
        app.world_mut()
            .spawn((Body::standing(), Transform::default()));
        let creature = app
            .world_mut()
            .spawn(Transform::from_xyz(100.0, 0.0, 0.0))
            .id();
        let show = |app: &mut App, functions: &[&str]| {
            let root = app.world_mut().spawn(Visibility::default()).id();
            app.insert_resource(Shown {
                root,
                creature,
                functions: functions.iter().map(|&f| f.to_owned()).collect(),
                text: String::new(),
                labels: Vec::new(),
            });
            root
        };
        let press = |app: &mut App| {
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            app.update();
            let mut buttons = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
            buttons.release(MouseButton::Left);
            buttons.clear();
            app.world().contains_resource::<Shown>()
        };
        let set = |app: &mut App, play: Play| {
            app.world_mut().resource_mut::<NextState<Play>>().set(play);
            app.update();
            app.update();
        };
        let drawn = |app: &App, root: Entity| *app.world().get::<Visibility>(root).unwrap();
        app.update();
        // Under the menu over a game (#2877) a press leaves the window, which the menu hides.
        let root = show(&mut app, &[]);
        set(&mut app, Play::Paused);
        assert!(press(&mut app));
        assert_eq!(drawn(&app, root), Visibility::Hidden);
        // Under the key P's pause and the scripts' own, which show no menu, it is drawn.
        app.world_mut().resource_mut::<Busy>().0 = true;
        assert!(press(&mut app));
        assert_eq!(drawn(&app, root), Visibility::Inherited);
        app.world_mut().resource_mut::<Busy>().0 = false;
        app.world_mut().resource_mut::<Still>().0 = true;
        app.update();
        assert_eq!(drawn(&app, root), Visibility::Inherited);
        app.world_mut().resource_mut::<Still>().0 = false;
        app.update();
        assert_eq!(drawn(&app, root), Visibility::Hidden);
        // The game going on, it is drawn again and a press closes it.
        set(&mut app, Play::Running);
        assert_eq!(drawn(&app, root), Visibility::Inherited);
        assert!(!press(&mut app));
        // One with buttons stays through a press beside them.
        show(&mut app, &["exit01"]);
        assert!(press(&mut app));
    }

    #[test]
    fn a_tips_picture_is_the_clients_image_of_its_entry() {
        // The Seraphim's tips: the first's 1042, the chest's 1030, her equipment's 1031 and
        // her class's 1035, which `Dialog:Hilfetext` leaves for the next (#2418).
        assert_eq!(tip_image(1042, 1), Some(0x456));
        assert_eq!(tip_image(1030, 1), Some(0x44A));
        assert_eq!(tip_image(1031, 1), Some(0x44B));
        assert_eq!(tip_image(1035, 1), Some(0x44F));
        // The table's single cases and its ends.
        assert_eq!(tip_image(0x400, 1), Some(0x411));
        assert_eq!(tip_image(0x402, 1), Some(0x417));
        assert_eq!(tip_image(0x413, 1), Some(0x419));
        assert_eq!(tip_image(0x423, 1), Some(0x47A));
        assert_eq!(tip_image(0x42A, 1), Some(0x6D9));
        // The hero's class's portrait, the Vampiress's night form as her day's.
        assert_eq!(tip_image(0x457, 3), Some(0x3C3));
        assert_eq!(tip_image(0x457, 7), Some(0x3C8));
        assert_eq!(tip_image(0x457, 10), None);
        // Any other value, a creature's mark among them, shows none.
        for icon in [0, 1, 0x3FF, 0x42B, 0x456, -1] {
            assert_eq!(tip_image(icon, 1), None, "{icon}");
        }
    }

    #[test]
    fn a_play_sounds_name_takes_the_prefix_it_lacks() {
        // A dialogue's voice-over names its text's key, an announcement its sound's whole name.
        assert_eq!(
            sound_name("NPC_Audio_Novizin"),
            "SOUND_FX_NPC_AUDIO_NOVIZIN"
        );
        assert_eq!(
            sound_name("SOUND_FX_SCSERA_AUDIO_6_3_2"),
            "SOUND_FX_SCSERA_AUDIO_6_3_2"
        );
        assert_eq!(sound_name("sound_fxthunder"), "SOUND_FXTHUNDER");
    }

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
    fn a_trader_without_a_dialogue_opens_its_window() {
        let trader = Mind::new(Vec3::ZERO, 0, (false, true), None, [70.0, 180.0], (692, 3));
        let speech = |trades| speech_of((None, &trader, Some(&Trades(trades))), None, || None);
        // The smith's bit before the merchant's, the merchant's before the master's (`0x52AB70`).
        assert_eq!(
            speech(opensacred_rules::quests::MERCHANT),
            Some(Speech::Merchant)
        );
        assert_eq!(
            speech(opensacred_rules::quests::MERCHANT | opensacred_rules::quests::WIZARD),
            Some(Speech::Merchant)
        );
        assert_eq!(
            speech(opensacred_rules::quests::WIZARD),
            Some(Speech::Master)
        );
        assert_eq!(speech(opensacred_rules::quests::SMITH), Some(Speech::Smith));
        assert_eq!(
            speech(opensacred_rules::quests::SMITH | opensacred_rules::quests::MERCHANT),
            Some(Speech::Smith)
        );
        // A dialogue goes first.
        let own = Talk("auftrag10".into());
        assert_eq!(
            speech_of(
                (
                    Some(&own),
                    &trader,
                    Some(&Trades(opensacred_rules::quests::MERCHANT))
                ),
                None,
                || None
            ),
            Some(Speech::Dialogue("auftrag10".into()))
        );
    }

    #[test]
    fn a_hero_from_afar_walks_within_the_reach_to_talk() {
        // The Dark Elf (80) and the elf archer of his start (130): a reach of 105, 157.5 at 1.5
        // times (#1636).
        let reach = minds::reach_f32(80, 130);
        // Within 1.5 times the reach as the talk begins, it opens where the hero stands.
        assert!(opens(157.0, reach, false, false));
        // From afar the hero walks on past 1.5 times the reach, to within the reach.
        assert!(!opens(157.0, reach, true, false));
        assert!(opens(105.0, reach, true, false));
        // A walk that ends short of the reach opens it within 1.5 times the reach, not beyond.
        assert!(opens(157.0, reach, true, true));
        assert!(!opens(158.0, reach, true, true));
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
    fn a_texts_colours_tags_paint_its_words() {
        let yellow = crate::tooltip::argb(0xFFFF_FF80);
        let red = crate::tooltip::argb(0xFFFF_0000);
        let run = |words: &str, colour: Color| (words.to_owned(), colour);
        // The dream's shape: a title, the tag, and paragraphs after it all yellow (#2420).
        assert_eq!(
            painted("<n> Title<color=FFFFFF80><n><n>One.<n>Two."),
            [run("\n Title", WORDS), run("\n\nOne.\nTwo.", yellow)]
        );
        // White again by a tag of white, as before a key's name.
        assert_eq!(
            painted("<color=FFFFFF80>Name <color=FFFFFFFF>[W]"),
            [
                run("Name ", yellow),
                run("[W]", crate::tooltip::argb(0xFFFF_FFFF))
            ]
        );
        // A line's colour ends with its line, a word's with its word.
        assert_eq!(
            painted("<colorl=FFFF0000>a b<n>c <colorw=FFFFFF80>d e"),
            [
                run("a b\n", red),
                run("c ", WORDS),
                run("d", yellow),
                run(" e", WORDS)
            ]
        );
        // Other tags kept as they stand, an unclosed one too.
        assert_eq!(painted("hm <sigh> on <n"), [run("hm <sigh> on <n", WORDS)]);
        assert_eq!(painted(""), []);
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
