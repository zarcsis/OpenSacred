//! The cursor's looks over the world (#1147; docs/re/cursor.md): at each frame of a game, what
//! lies under the cursor as a left click takes it (`attack`, `talk`, `pickup`, `doors`) chooses
//! the original's picture, as `cEngine::updateWorldCursor` (`0x611E70`) does: over a living foe
//! the sword, the bow with a bow or a crossbow in a hand, the pistol with a gun (`MOUSE_ATTACK`,
//! `MOUSE_ATTACKA`, `MOUSE_GUN`); over another creature a trader's sign by its trades, else the
//! speech bubble where it has a dialogue (`MOUSE_TRADER`, `MOUSE_COMBO`, `MOUSE_BLACKSMITH`,
//! `MOUSE_HORSETRADER`, `MOUSE_DIALOG04`); the hand over an item, lying on the ground or still in
//! flight (#1647), over a door, over a container that opens (#1380) and over a hidden stash or a
//! mouse event of the scripts (#1518, `stashes`; `mouse_action`), and over the teleports, the
//! stairs and the cells before them that `bin/treppe.bin` sends a click on to them (`0x616AD0`,
//! `0xAB4498`, #2508); else, over the interface and outside a game, the arrow (`MOUSE_MAIN`); and
//! the pin while the world map's pin's mode holds (`MOUSE_PUSHPIN`, #1569), else at its screen's
//! edges their arrows (`MOUSE_DIRN` …, #2933). `hud` puts the picture on the window. Beside the hand over an item, the item's name, and over a stash its text.
//! The same choice tells the foe under the cursor, whose panel shows ([`Foe`], #1459), what the
//! hover highlights ([`Highlit`], #1460) and whether the cursor glows, the right button's art
//! ready for what lies under it ([`Glow`], #1461).
//!
//! Over a horse that is no foe, its owner's and the riding's look and its description beside the
//! cursor (#2936, [`horse_look`]).
//!
//! Not followed yet: the glow over a friend or the dead for an art on one (the launch types 2 and
//! 4); the name's red for some objects (`+0x14` bit `0x20`, not read).

use bevy::picking::hover::HoverMap;
use bevy::prelude::*;
use opensacred_rules::drops::Dropped;
use opensacred_rules::fight::Holds;
use opensacred_rules::quests;

use crate::ai::Mind;
use crate::buildings::Buildings;
use crate::drops::{Lying, Tables};
use crate::fight::{Health, Holding};
use crate::forms::Forms;
use crate::game::{AppState, FONT};
use crate::hero::Body;
use crate::hud::Pointer;
use crate::lines::Texts;
use crate::npcs::{Dies, Door, Npcs, Talk, TextName, Trades};
use crate::terrain::Heights;
use crate::{paths, pick};

/// A picture of the cursor's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Look {
    /// The arrow, the menus' and the world's where nothing else is chosen.
    #[default]
    Arrow,
    Hand,
    Sword,
    Bow,
    Gun,
    Talk,
    Merchant,
    Combo,
    Smith,
    HorseTrader,
    /// The world map's pin's mode (#1569).
    Pushpin,
    /// The world map's edge, its arrow (#2933).
    Edge(Edge),
    /// A horse to ride, another's to steal, one the hero may not ride, and another's that it may
    /// not ride to steal (#2936).
    Ride,
    StealHorse,
    CantRide,
    StealHorseFail,
}

/// An edge of the world map's screen, or a corner, whose arrow the cursor shows as the map
/// scrolls that way (`0x6C7920`, `MOUSE_DIRNW` …).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Edge {
    NorthWest,
    North,
    NorthEast,
    West,
    East,
    SouthWest,
    South,
    SouthEast,
}

impl Edge {
    /// Its arrow's texture and the arrow's point within it, the opposite of the offset at which
    /// the map pushes it (`0x6C7540`).
    fn arrow(self) -> (&'static str, (u16, u16)) {
        match self {
            Edge::NorthWest => ("mouse_dirnw", (0, 0)),
            Edge::North => ("mouse_dirn", (16, 0)),
            Edge::NorthEast => ("mouse_dirne", (32, 0)),
            Edge::West => ("mouse_dirw", (0, 16)),
            Edge::East => ("mouse_dire", (40, 16)),
            Edge::SouthWest => ("mouse_dirsw", (0, 32)),
            Edge::South => ("mouse_dirs", (16, 40)),
            Edge::SouthEast => ("mouse_dirse", (32, 32)),
        }
    }
}

impl Look {
    pub const ALL: [Look; 23] = [
        Look::Arrow,
        Look::Hand,
        Look::Sword,
        Look::Bow,
        Look::Gun,
        Look::Talk,
        Look::Merchant,
        Look::Combo,
        Look::Smith,
        Look::HorseTrader,
        Look::Pushpin,
        Look::Edge(Edge::NorthWest),
        Look::Edge(Edge::North),
        Look::Edge(Edge::NorthEast),
        Look::Edge(Edge::West),
        Look::Edge(Edge::East),
        Look::Edge(Edge::SouthWest),
        Look::Edge(Edge::South),
        Look::Edge(Edge::SouthEast),
        Look::Ride,
        Look::StealHorse,
        Look::CantRide,
        Look::StealHorseFail,
    ];

    /// Its texture's name, as the converter names the file.
    pub fn texture(self) -> &'static str {
        match self {
            Look::Arrow => opensacred_data::texture::CURSOR,
            Look::Hand => "mouse_action",
            Look::Sword => "mouse_attack",
            Look::Bow => "mouse_attacka",
            Look::Gun => "mouse_gun",
            Look::Talk => "mouse_dialog04",
            Look::Merchant => "mouse_trader",
            Look::Combo => "mouse_combo",
            Look::Smith => "mouse_blacksmith",
            Look::HorseTrader => "mouse_horsetrader",
            Look::Pushpin => "mouse_pushpin",
            Look::Edge(edge) => edge.arrow().0,
            Look::Ride => "mouse_ride",
            Look::StealHorse => "mouse_stealhorse",
            Look::CantRide => "mouse_cantride",
            Look::StealHorseFail => "mouse_stealhorsefail",
        }
    }

    /// Its point within its picture, at the scale 1: the pin's middle, which the original
    /// draws 16 pixels up and left of the pointer, and each edge's arrow's tip (`0x6C7540`), the
    /// others' corner.
    pub fn spot(self) -> (u16, u16) {
        match self {
            Look::Pushpin => (16, 16),
            Look::Edge(edge) => edge.arrow().1,
            _ => (0, 0),
        }
    }

    /// The side of the square of its texture that the cursor shows: the edges' arrows' up to
    /// 43 × 43 within 48, the others' within the top left 32 (docs/re/cursor.md, "The
    /// textures").
    pub fn side(self) -> usize {
        match self {
            Look::Edge(_) => 48,
            _ => 32,
        }
    }

    /// Its name in the overlay and the state.
    pub fn name(self) -> &'static str {
        match self {
            Look::Arrow => "arrow",
            Look::Hand => "hand",
            Look::Sword => "sword",
            Look::Bow => "bow",
            Look::Gun => "gun",
            Look::Talk => "talk",
            Look::Merchant => "merchant",
            Look::Combo => "combo",
            Look::Smith => "smith",
            Look::HorseTrader => "horse-trader",
            Look::Pushpin => "pushpin",
            Look::Edge(Edge::NorthWest) => "edge-nw",
            Look::Edge(Edge::North) => "edge-n",
            Look::Edge(Edge::NorthEast) => "edge-ne",
            Look::Edge(Edge::West) => "edge-w",
            Look::Edge(Edge::East) => "edge-e",
            Look::Edge(Edge::SouthWest) => "edge-sw",
            Look::Edge(Edge::South) => "edge-s",
            Look::Edge(Edge::SouthEast) => "edge-se",
            Look::Ride => "ride",
            Look::StealHorse => "steal-horse",
            Look::CantRide => "cant-ride",
            Look::StealHorseFail => "steal-horse-fail",
        }
    }
}

/// The cursor over a horse that is no foe (`0x611E70`'s horse branch, #2936), its owner `owner`,
/// whether the hero `may` ride it, whether the hero `rides` now and whether Shift is held: where
/// it may not, another NPC's the theft refused and any other's the horseshoe crossed; where it
/// may, a trader's the horse trader's, another's the theft, and the hero's own or one of no owner
/// the ride, but while the hero rides only with Shift held, else the arrow.
fn horse_look(owner: crate::riding::Owner, may: bool, rides: bool, shift: bool) -> Look {
    use crate::riding::Owner;
    match (may, owner) {
        (false, Owner::Another) => Look::StealHorseFail,
        (false, _) => Look::CantRide,
        (true, Owner::Trader) => Look::HorseTrader,
        (true, Owner::Another) => Look::StealHorse,
        (true, Owner::None | Owner::Hero) if rides && !shift => Look::Arrow,
        (true, Owner::None | Owner::Hero) => Look::Ride,
    }
}

/// The horse under the cursor whose description shows beside it: one that is no hero's own
/// (`0x426650`), alive (#2936).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Described(pub Option<Entity>);

/// The description's left side's middle from the cursor, in the original's pixels (`0x611E70`:
/// the point 32 above the cursor; **measured**: the panel's left 24 right of it, as the items'
/// names beside the cursor).
const DESCRIBED_AT: Vec2 = Vec2::new(24.0, -32.0);

/// The description of the horse under the cursor, beside it (`0x55C9F0`, the style `0x20B`),
/// made as the horse comes under it and taken away as it leaves; the window's point the
/// original's screen's.
#[derive(Component)]
struct HorseTip;

/// Shows the description of the horse that [`Described`] names beside the cursor.
fn describe_horse(
    mut commands: Commands,
    described: Res<Described>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (images, assets, texts): (
        Res<crate::bar::UiImages>,
        Res<AssetServer>,
        Option<Res<Texts>>,
    ),
    sheet: crate::riding::HorseSheet,
    (parents, tips): (Query<&ChildOf>, Query<Entity, With<HorseTip>>),
    mut shown: Local<Option<(Entity, Vec2)>>,
) {
    let at = crate::inventory::screen_point(&pointer, &ui_scale, &windows);
    let wanted = described.0.zip(at);
    if wanted == *shown {
        return;
    }
    for tip in &tips {
        let root = parents.get(tip).map_or(tip, ChildOf::parent);
        commands.entity(root).despawn();
    }
    *shown = wanted;
    let Some((horse, at)) = wanted else {
        return;
    };
    let Some(lines) = sheet.lines(horse, texts.as_deref()) else {
        return;
    };
    let panel = crate::tooltip::spawn_lines_tip_beside(
        &mut commands,
        (&images, &assets),
        (&lines, at + DESCRIBED_AT),
    );
    commands.entity(panel).insert(HorseTip);
}

/// The picture the cursor shows.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shown(pub Look);

/// The living foe under the cursor, nearest its line of sight, whose panel the interface shows
/// (`foe_panel`, #1459).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Foe(pub Option<Entity>);

/// What lies under the cursor that the original highlights, a foe or a container under the hand
/// (`+0x14` bit `0x20000000`; `hover`, #1460).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Highlit(pub Option<Entity>);

/// Whether the cursor glows (`hud`, #1461): the right button's selected art may be made now and
/// admits what lies under the cursor, nothing on the cursor (`0x612DE3`: `0x580DF0`, `0x51D840`).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Glow(pub bool);

/// What lies under the cursor that an art's launch asks for ([`admits`]): a foe, the hero
/// himself nearer the cursor's line than a foe, and a dead creature, as the right button picks
/// them (`making::aim`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Under {
    foe: bool,
    hero: bool,
    dead: bool,
}

/// Whether an art of the launch type `launch` (`making::launch`) admits what lies `under` the
/// cursor (`0x51D840`), as the right button makes it there (`making::aim`): an art on an enemy a
/// foe; on a friend (the type 2) the hero himself, and on the dead (the type 4) a dead creature
/// (#2938); one on the ground, on its maker or of any target anything.
fn admits(launch: Option<u8>, under: Under) -> bool {
    match launch {
        Some(1) => under.foe,
        Some(2) => under.hero,
        Some(4) => under.dead,
        Some(0 | 3 | 5) => true,
        _ => false,
    }
}

/// What is under the hand, whose name shows beside the cursor: an item, and the name of the
/// game's texts that the scripts gave it (`npcs::TextName`), or a hidden stash's text's name
/// alone (#1518).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
struct Named(Option<(Option<Dropped>, Option<String>)>);

impl Named {
    /// The item's name ([`item_name`]), of the item maker's name where `tables` are given; a
    /// stash's text.
    fn shown<'a>(&self, texts: &'a Texts, tables: Option<&Tables>) -> Option<&'a str> {
        match self.0.as_ref()? {
            (Some(dropped), text) => item_name(texts, tables, dropped, text.as_deref()),
            (None, text) => texts
                .named(text.as_deref()?)
                .filter(|name| !name.is_empty()),
        }
    }
}

/// The name of the item `dropped`, its object's key's text (`0x44A250`): the text of its text
/// name `text` where the scripts gave it one, which may have none, else of the key its object
/// came with where it has a text ([`Dropped::name_key`], a carried hero's item, #2390), else of
/// the name that the item maker gave it where it made it ([`Tables::made_name`], #1424), else its
/// type's; none for an empty one.
pub(crate) fn item_name<'a>(
    texts: &'a Texts,
    tables: Option<&Tables>,
    dropped: &Dropped,
    text: Option<&str>,
) -> Option<&'a str> {
    if text.is_none()
        && dropped.name_key != 0
        && let Some(name) = texts.id(dropped.name_key & 0x7FFF_FFFF)
    {
        return Some(name).filter(|name| !name.is_empty());
    }
    let key = text.map(str::to_owned).unwrap_or_else(|| {
        tables
            .and_then(|tables| tables.made_name(texts, dropped))
            .unwrap_or_else(|| dropped.item.to_string())
    });
    texts.named(&key).filter(|name| !name.is_empty())
}

/// The item's name's place from the cursor, in the original's pixels, and its size: the
/// tooltips' (seen, #1777: a set's helm's name of 21 characters 162 pixels wide at 1024 × 768,
/// as the tooltip's line of the same name).
const NAME_OFFSET: Vec2 = Vec2::new(24.0, 16.0);
const NAME_SIZE: f32 = crate::tooltip::FONT_SIZE;

/// The node of the item's name beside the cursor.
#[derive(Component)]
struct NameTag;

/// The cursor over a living foe, by what the hero holds: a gun in either hand the pistol
/// (`0x54A340`), else a bow or a crossbow the bow (`0x54A2A0`), but in the Vampiress's night's
/// form, else the sword.
fn foe_look(holds: Option<&Holds>, by_night: bool) -> Look {
    match holds {
        Some(holds) if holds.gun => Look::Gun,
        Some(holds) if holds.ranged && !by_night => Look::Bow,
        _ => Look::Sword,
    }
}

/// The cursor over a creature that is no foe, by its trades (`+0x200`) and whether it has a
/// dialogue, in the original's order: the merchants, the combat masters, the smiths, the horse
/// trader, then the speech bubble.
fn creature_look(trades: u32, dialogue: bool) -> Option<Look> {
    [
        (quests::MERCHANT, Look::Merchant),
        (quests::WIZARD, Look::Combo),
        (quests::SMITH, Look::Smith),
        (quests::TRADER, Look::HorseTrader),
    ]
    .into_iter()
    .find(|&(bit, _)| trades & bit != 0)
    .map(|(_, look)| look)
    .or(dialogue.then_some(Look::Talk))
}

/// The cursor's look at each frame: the arrow outside a game, while a dialogue's window shows and
/// over the interface; else by what the cursor picks, as a click takes it (`under_cursor`,
/// `0x626C50`): a living foe its weapons' look, a creature of a dialogue or a trader its own, an
/// item, lying or in flight, a door, a container that opens and an event of a box the hand; with
/// nothing picked, a hidden stash without a box near the cursor's line of sight the hand, then
/// the teleport or the stairs of the cell a click sends the hero to (`pick`); the pin whatever
/// lies there while the world map's pin's mode holds.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their \
              filters"
)]
fn choose(
    (mut shown, mut named, mut foe_shown, mut highlit): (
        ResMut<Shown>,
        ResMut<Named>,
        ResMut<Foe>,
        ResMut<Highlit>,
    ),
    (state, window, pointer, map): (
        Option<Res<State<AppState>>>,
        Option<Res<crate::talk::Shown>>,
        Res<Pointer>,
        Option<Res<crate::world_map::Map>>,
    ),
    (hovered, nodes, held): (
        Option<Res<HoverMap>>,
        Query<(), With<Node>>,
        Option<Res<crate::frame::HeldBack>>,
    ),
    (under_cursor, hero_ids, owners, mut described, keys): (
        Res<crate::under_cursor::UnderCursor>,
        Query<Entity, With<Body>>,
        crate::riding::HorseOwners,
        ResMut<Described>,
        crate::controls::Keys,
    ),
    cameras: Query<(&Camera, &GlobalTransform), crate::iso::Eye<With<Camera>>>,
    (npcs, forms, lines): (Option<Res<Npcs>>, Option<Res<Forms>>, crate::talk::Lines),
    creatures: Query<
        (
            Entity,
            &GlobalTransform,
            &Mind,
            &Health,
            Option<&Talk>,
            Option<&Trades>,
        ),
        Without<Dies>,
    >,
    (items, doors, containers, stashes, corpses): (
        Query<(&Lying, Option<&TextName>)>,
        Query<(), With<Door>>,
        Query<&crate::containers::Container>,
        Query<(&GlobalTransform, &crate::stashes::Stash)>,
        Query<(), With<crate::npcs::Corpse>>,
    ),
    (heights, buildings, data, dungeons, ways): (
        Option<Res<Heights>>,
        Option<Res<Buildings>>,
        Option<Res<crate::game::Data>>,
        Option<Res<crate::daynight::Dungeons>>,
        Query<&crate::crowd::InTheWay>,
    ),
    heroes: Query<(
        Option<&Holding>,
        &Transform,
        &Body,
        Option<&crate::effects::Effects>,
        Option<&crate::riding::Seat>,
    )>,
    (selected, arts, pack, mut glow): (
        Res<crate::making::Selected>,
        Option<Res<crate::arts::HeroArts>>,
        Option<Res<crate::pickup::Pack>>,
        ResMut<Glow>,
    ),
) {
    let in_game = state.is_some_and(|state| *state.get() == AppState::InGame);
    // Over a node of the interface, or where a window holds the world's clicks back (the quest
    // book's pages and bookmarks, which the pointer's picking passes through, #2507).
    let over_interface = held.is_some_and(|held| held.held())
        || hovered.is_some_and(|hovered| {
            hovered
                .values()
                .flat_map(|hits| hits.keys())
                .any(|&entity| nodes.contains(entity))
        });
    let ray = pointer
        .0
        .zip(cameras.single().ok())
        .and_then(|(cursor, (camera, view))| camera.viewport_to_world(view, cursor).ok());
    let mut item = None;
    let mut foe_under = None;
    let mut container_under = None;
    let mut under = Under::default();
    let look = match ray {
        Some(ray) if in_game && window.is_none() && !over_interface => {
            let picked = under_cursor.0;
            let by_night = forms.as_deref().is_some_and(Forms::by_night);
            let hero = heroes.iter().next();
            let holding = hero.and_then(|(holding, ..)| holding);
            // A living creature that the cursor picks: a foe as `attack::aim` takes one, alive,
            // hostile and of no dialogue, else its own look.
            let creature = picked.and_then(|entity| {
                let (_, _, mind, health, talk, trades) = creatures.get(entity).ok()?;
                (health.current > 0).then_some((entity, mind, talk, trades))
            });
            foe_under = creature
                .filter(|(_, mind, talk, _)| talk.is_none() && mind.hostile())
                .map(|(entity, ..)| entity);
            under = Under {
                foe: foe_under.is_some(),
                hero: picked.is_some_and(|entity| hero_ids.contains(entity)),
                dead: picked.is_some_and(|entity| corpses.contains(entity)),
            };
            let creature_look =
                |(entity, mind, talk, trades): (Entity, &Mind, Option<&Talk>, Option<&Trades>)| {
                    let dialogue = crate::talk::dialogue_of(talk, mind, npcs.as_deref()).is_some()
                        || (crate::talk::generic(talk, mind) && lines.line(entity, mind).is_some());
                    creature_look(trades.map_or(0, |trades| trades.0), dialogue)
                };
            let foe = foe_under.map(|_| foe_look(holding.map(|holding| &holding.0), by_night));
            // A horse that is no foe, by its owner and whether the hero may ride it; its
            // description beside the cursor unless it is the hero's own (#2936).
            let horse = creature
                .filter(|_| foe.is_none())
                .and_then(|(entity, ..)| Some((entity, owners.of(entity)?)));
            let rides = hero
                .and_then(|(.., seat)| seat)
                .is_some_and(|seat| seat.ridden().is_some());
            let shift = keys.pressed(crate::controls::Action::Walk);
            described.set_if_neq(Described(
                horse
                    .filter(|(_, (owner, _))| *owner != crate::riding::Owner::Hero)
                    .map(|(entity, _)| entity),
            ));
            let horse = horse.map(|(_, (owner, may))| horse_look(owner, may, rides, shift));
            foe.or(horse)
                .or_else(|| creature.and_then(creature_look))
                .or_else(|| {
                    let (lying, text) = items.get(picked?).ok()?;
                    item = Some((Some(lying.dropped.clone()), text.map(|text| text.0.clone())));
                    Some(Look::Hand)
                })
                .or_else(|| doors.contains(picked?).then_some(Look::Hand))
                // A container that opens, not one emptied (`0x612843`, #1380).
                .or_else(|| {
                    let entity = picked?;
                    containers.get(entity).ok()?.opens().then(|| {
                        container_under = Some(entity);
                        Look::Hand
                    })
                })
                // An event of a box picked: the hand and its text (`0x612909`, #1586); with nothing
                // picked, a hidden stash without one near the cursor's line of sight (#1518).
                .or_else(|| {
                    let stash = match picked {
                        Some(entity) => stashes.get(entity).ok().map(|(_, stash)| stash),
                        None => stashes
                            .iter()
                            .filter(|(_, stash)| stash.boxed.is_none())
                            .filter_map(|(at, stash)| {
                                Some((crate::stashes::picked(ray, at.translation())?, stash))
                            })
                            .min_by(|a, b| a.0.total_cmp(&b.0))
                            .map(|(_, stash)| stash),
                    }?;
                    item = Some((None, Some(stash.text.clone())));
                    Some(Look::Hand)
                })
                .or_else(|| {
                    let (heights, buildings) = (heights.as_deref()?, buildings.as_deref()?);
                    let (_, transform, body, ..) = hero?;
                    let height = transform.translation.y;
                    let picked = pick::pick(ray, heights, buildings, (height, body.level()))?;
                    let cell = paths::cell_of(picked.point.x, picked.point.z);
                    let floor = heights.floor_number(picked.level);
                    let sends = data
                        .as_deref()
                        .is_some_and(|data| data.0.tables.stair_to(cell, floor).is_some());
                    (sends || heights.beckons(cell, picked.level)).then_some(Look::Hand)
                })
                .unwrap_or_default()
        }
        _ => Look::Arrow,
    };
    // The world map's pin's mode shows the pin (`0x6C8140`), and its edges their arrows
    // (`0x6C7920`, #2933).
    let look = match map.as_deref() {
        Some(map) if map.pin_mode() => Look::Pushpin,
        Some(map) => map.edge().map_or(look, Look::Edge),
        None => look,
    };
    shown.set_if_neq(Shown(look));
    named.set_if_neq(Named(item.filter(|_| look == Look::Hand)));
    foe_shown.set_if_neq(Foe(foe_under));
    highlit.set_if_neq(Highlit(foe_under.or(container_under)));
    let in_world = ray.is_some() && in_game && window.is_none() && !over_interface;
    let glows = in_world
        && pack.as_ref().is_none_or(|pack| pack.held.is_none())
        && selected.0.zip(arts.as_deref()).is_some_and(|(art, arts)| {
            let hero = heroes.iter().next();
            let effects = hero.and_then(|(.., effects, _)| effects);
            let riding =
                hero.and_then(|(holding, .., seat)| crate::riding::riding_with(seat, holding));
            // The art of the hero's form for the selected one (`0x552370`).
            let art = opensacred_rules::arts::right_button(art, arts.kind, |key| {
                effects.is_some_and(|effects| effects.holds_any(&[key]))
            });
            // The mortar's point, the foe's place, else the ground's under the cursor at the
            // hero's height, on the hero's floor and reached where it must be (`0x51D840`).
            let launch = crate::making::launch(arts, art);
            let mortar =
                opensacred_rules::arts::style(art) == Some(opensacred_rules::arts::Style::Mortar);
            // The art's point, the foe's place for the mortar, else the ground's under the cursor
            // at the hero's height, on the hero's floor and reached where it must be: the
            // mortar's way, a point's of the launch type 3 (`0x51D840`).
            let reached = || {
                let (Some((_, transform, body, ..)), Some(heights), Some(ray)) =
                    (hero, heights.as_deref(), ray)
                else {
                    return false;
                };
                let (here, level) = (transform.translation, body.level());
                let ground = || {
                    let along = (here.y - ray.origin.y) / ray.direction.y;
                    (ray.origin + ray.direction.as_vec3() * along, level)
                };
                let walkable = |cell| heights.walkable_on(cell, level);
                if !mortar {
                    return crate::making::point_reached((here, level), ground(), walkable);
                }
                let point = foe_under
                    .and_then(|foe| {
                        let (_, at, ..) = creatures.get(foe).ok()?;
                        let on = ways.get(foe).map_or(level, |way| way.level);
                        Some((at.translation(), on))
                    })
                    .unwrap_or_else(ground);
                let dungeon = dungeons
                    .as_deref()
                    .is_some_and(|dungeons| dungeons.holds(here.x, here.z));
                crate::making::mortar_reaches((here, level), point, dungeon, walkable)
            };
            crate::making::may_make(arts, art, (effects, pack.as_deref(), riding)).is_ok()
                && admits(launch, under)
                && (!(mortar || launch == Some(3)) || reached())
        });
    glow.set_if_neq(Glow(glows));
}

/// The item's name beside the cursor while the hand is over it (`0x611E70`, as it draws): its
/// text of the game's texts by the name the scripts gave it, else by its type ([`Named::shown`]),
/// in its name's colour (its query 2, as its tooltip's first line, #1777), 24 of the original's
/// pixels right of the cursor and 16 down; none without a name.
fn name_tag(
    mut commands: Commands,
    named: Res<Named>,
    pointer: Res<Pointer>,
    ui_scale: Res<UiScale>,
    (texts, tables, assets): (Option<Res<Texts>>, Option<Res<Tables>>, Res<AssetServer>),
    mut tags: Query<(Entity, &mut Node, &mut Text, &mut TextColor), With<NameTag>>,
) {
    let name = texts
        .as_deref()
        .and_then(|texts| named.shown(texts, tables.as_deref()));
    let (Some(name), Some(at)) = (name, pointer.0) else {
        for (tag, ..) in &tags {
            commands.entity(tag).despawn();
        }
        return;
    };
    let colour = named
        .0
        .as_ref()
        .and_then(|(dropped, _)| dropped.as_ref())
        .zip(tables.as_deref())
        .map_or(Color::WHITE, |(dropped, tables)| {
            crate::tooltip::argb(crate::tooltip::name_colour(tables, dropped))
        });
    // The window's pixels in the interface's, which `UiScale` scales (`hud::scale`).
    let at = at / ui_scale.0 + NAME_OFFSET;
    if let Ok((_, mut node, mut text, mut text_colour)) = tags.single_mut() {
        node.left = Val::Px(at.x);
        node.top = Val::Px(at.y);
        if text.0 != name {
            name.clone_into(&mut text.0);
        }
        if text_colour.0 != colour {
            text_colour.0 = colour;
        }
        return;
    }
    commands.spawn((
        NameTag,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(at.x),
            top: Val::Px(at.y),
            ..default()
        },
        Text::new(name),
        TextFont {
            font: assets.load(FONT).into(),
            font_size: FontSize::Px(NAME_SIZE),
            ..default()
        },
        TextColor(colour),
        GlobalZIndex(10),
        Pickable::IGNORE,
        DespawnOnExit(AppState::InGame),
    ));
}

/// The cursor's looks.
pub struct CursorsPlugin;

impl Plugin for CursorsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shown>()
            .init_resource::<Named>()
            .init_resource::<Foe>()
            .init_resource::<Highlit>()
            .init_resource::<Glow>()
            .init_resource::<Described>()
            .add_systems(Update, (choose, name_tag, describe_horse).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_horses_look_is_its_owners_and_the_riding() {
        use crate::riding::Owner;
        // Not to be ridden: another NPC's the theft refused, any other's the horseshoe crossed.
        assert_eq!(
            horse_look(Owner::Another, false, false, false),
            Look::StealHorseFail
        );
        assert_eq!(
            horse_look(Owner::Trader, false, false, false),
            Look::CantRide
        );
        assert_eq!(horse_look(Owner::None, false, false, false), Look::CantRide);
        // To be ridden: the trader's, another's, and the ride.
        assert_eq!(
            horse_look(Owner::Trader, true, false, false),
            Look::HorseTrader
        );
        assert_eq!(
            horse_look(Owner::Another, true, true, false),
            Look::StealHorse
        );
        assert_eq!(horse_look(Owner::None, true, false, false), Look::Ride);
        assert_eq!(horse_look(Owner::Hero, true, false, false), Look::Ride);
        // While riding, the ride only with Shift held.
        assert_eq!(horse_look(Owner::Hero, true, true, false), Look::Arrow);
        assert_eq!(horse_look(Owner::Hero, true, true, true), Look::Ride);
    }

    #[test]
    fn the_cursor_glows_over_what_the_arts_launch_admits() {
        let foe = Under {
            foe: true,
            ..Under::default()
        };
        let hero = Under {
            hero: true,
            ..Under::default()
        };
        let dead = Under {
            dead: true,
            ..Under::default()
        };
        let nothing = Under::default();
        assert!(admits(Some(1), foe));
        assert!(!admits(Some(1), nothing));
        assert!(admits(Some(3), nothing) && admits(Some(5), nothing) && admits(Some(0), nothing));
        // On a friend the hero himself, on the dead a corpse (#2938).
        assert!(admits(Some(2), hero) && !admits(Some(2), foe) && !admits(Some(2), nothing));
        assert!(admits(Some(4), dead) && !admits(Some(4), foe));
        assert!(!admits(None, foe));
    }

    #[test]
    fn a_foe_shows_the_weapon_held() {
        let holds = |ranged, gun| Holds {
            ranged,
            gun,
            ..Holds::default()
        };
        assert_eq!(foe_look(None, false), Look::Sword);
        assert_eq!(foe_look(Some(&holds(false, false)), false), Look::Sword);
        assert_eq!(foe_look(Some(&holds(true, false)), false), Look::Bow);
        assert_eq!(foe_look(Some(&holds(true, true)), false), Look::Gun);
        // The Vampiress by night fights with her claws, whatever she holds; a gun stays a gun.
        assert_eq!(foe_look(Some(&holds(true, false)), true), Look::Sword);
        assert_eq!(foe_look(Some(&holds(true, true)), true), Look::Gun);
    }

    #[test]
    fn an_item_the_scripts_named_goes_by_its_text_name_else_by_its_type() {
        use opensacred_data::text::{message, sacred_key};
        let texts = Texts::of(&format!(
            "{} = a type's name\n{} = a quest's name\n{} = {{\"\"}}\n{} = a made name\n",
            message("6001"),
            message("QUEST_PLANT"),
            message("EMPTY"),
            message("GEN_GR_FE_RING2")
        ));
        let named_by = |item, text: Option<&str>, name_key| {
            let dropped = Dropped {
                kind: opensacred_rules::drops::Kind::Other,
                item,
                level: 0,
                key: 16,
                quality: 0,
                amount: 1,
                values: None,
                named: None,
                socketed: Vec::new(),
                name_key,
                script: None,
            };
            Named(Some((Some(dropped), text.map(str::to_owned))))
        };
        let named = |item, text: Option<&str>| named_by(item, text, 0);
        assert_eq!(named(6001, None).shown(&texts, None), Some("a type's name"));
        assert_eq!(
            named(6001, Some("quest_plant")).shown(&texts, None),
            Some("a quest's name")
        );
        // A text name of no text names the item nothing, not by its type; nor does an empty one.
        assert_eq!(named(6001, Some("NO_TEXT")).shown(&texts, None), None);
        assert_eq!(named(6001, Some("EMPTY")).shown(&texts, None), None);
        assert_eq!(named(6002, None).shown(&texts, None), None);
        assert_eq!(Named(None).shown(&texts, None), None);
        // A carried hero's item goes by the key its file kept (#2390), with the bit 31 set, else
        // where it has no text by its type's; a text name the scripts gave it goes first.
        let ring = sacred_key(b"GEN_GR_FE_RING2") | 0x8000_0000;
        assert_eq!(
            named_by(6001, None, ring).shown(&texts, None),
            Some("a made name")
        );
        assert_eq!(
            named_by(6001, None, sacred_key(b"NO_TEXT") | 0x8000_0000).shown(&texts, None),
            Some("a type's name")
        );
        assert_eq!(
            named_by(6001, Some("quest_plant"), ring).shown(&texts, None),
            Some("a quest's name")
        );
    }

    #[test]
    fn a_creature_shows_its_trade_before_its_dialogue() {
        assert_eq!(creature_look(0, false), None);
        assert_eq!(creature_look(0, true), Some(Look::Talk));
        assert_eq!(creature_look(quests::MERCHANT, true), Some(Look::Merchant));
        assert_eq!(
            creature_look(quests::SMITH | quests::WIZARD, false),
            Some(Look::Combo)
        );
        assert_eq!(creature_look(quests::SMITH, false), Some(Look::Smith));
        assert_eq!(creature_look(quests::TRADER, true), Some(Look::HorseTrader));
    }

    #[test]
    fn every_look_has_its_own_texture_and_name() {
        let textures: std::collections::HashSet<_> = Look::ALL.map(Look::texture).into();
        let names: std::collections::HashSet<_> = Look::ALL.map(Look::name).into();
        assert_eq!(
            (textures.len(), names.len()),
            (Look::ALL.len(), Look::ALL.len())
        );
    }
}
