//! The cursor's looks over the world (#1147; docs/re/cursor.md): at each frame of a game, what
//! lies under the cursor as a left click takes it (`attack`, `talk`, `pickup`, `doors`) chooses
//! the original's picture, as `cEngine::updateWorldCursor` (`0x611E70`) does: over a living foe
//! the sword, the bow with a bow or a crossbow in a hand, the pistol with a gun (`MOUSE_ATTACK`,
//! `MOUSE_ATTACKA`, `MOUSE_GUN`); over another creature a trader's sign by its trades, else the
//! speech bubble where it has a dialogue (`MOUSE_TRADER`, `MOUSE_COMBO`, `MOUSE_BLACKSMITH`,
//! `MOUSE_HORSETRADER`, `MOUSE_DIALOG04`); the hand over an item, lying on the ground or still in
//! flight (#1647), and over a door (`mouse_action`), and over the stairs where a click sends the hero (`0x616AD0`); else,
//! over the interface and outside a game, the arrow (`MOUSE_MAIN`). `hud` puts the picture on the
//! window. Beside the hand over an item, the item's name.
//!
//! Not followed yet: the hand over the teleports and the scripts' spots, the glow of a right-button
//! art that may be made, the enemy's panel, the hover's highlight, the horses' looks and the map's
//! arrows; the name's red for some objects (`+0x14` bit `0x20`, not read).

use bevy::picking::hover::HoverMap;
use bevy::prelude::*;
use opensacred_rules::drops::Dropped;
use opensacred_rules::fight::Holds;
use opensacred_rules::quests;

use crate::ai::Mind;
use crate::attack;
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
}

impl Look {
    pub const ALL: [Look; 11] = [
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
        }
    }

    /// Its point within its picture, at the scale 1: the pin's middle, which the original
    /// draws 16 pixels up and left of the pointer (`0x6C7540`), the others' corner.
    pub fn spot(self) -> u16 {
        match self {
            Look::Pushpin => 16,
            _ => 0,
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
        }
    }
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

/// Whether an art of the launch type `launch` (`making::launch`) admits what lies under the
/// cursor, `foe` whether a foe does (`0x51D840`): an art on an enemy a foe, one on the ground, on
/// its maker or of any target anything; one on a friend or on the dead nothing, their targets not
/// followed for the glow.
fn admits(launch: Option<u8>, foe: bool) -> bool {
    match launch {
        Some(1) => foe,
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
/// name `text` where the scripts gave it one, which may have none, else of the name that the item
/// maker gave it where it made it ([`Tables::made_name`], #1424), else its type's; none for an
/// empty one.
pub(crate) fn item_name<'a>(
    texts: &'a Texts,
    tables: Option<&Tables>,
    dropped: &Dropped,
    text: Option<&str>,
) -> Option<&'a str> {
    let key = text.map(str::to_owned).unwrap_or_else(|| {
        tables
            .and_then(|tables| tables.made_name(texts, dropped))
            .unwrap_or_else(|| dropped.item.to_string())
    });
    texts.named(&key).filter(|name| !name.is_empty())
}

/// The item's name's place from the cursor, in the original's pixels, and its size.
const NAME_OFFSET: Vec2 = Vec2::new(24.0, 16.0);
const NAME_SIZE: f32 = 14.0;

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
/// over the interface; else by what lies nearest the cursor's line of sight, as a click picks it
/// (`attack::from_line`): a living foe first, then a creature of a dialogue or a trader, then an
/// item, lying or in flight, then a door, then the stairs of the cell a click sends the hero to
/// (`pick`).
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
    (hovered, nodes): (Option<Res<HoverMap>>, Query<(), With<Node>>),
    cameras: Query<(&Camera, &GlobalTransform), crate::iso::Eye<With<Camera>>>,
    (npcs, forms): (Option<Res<Npcs>>, Option<Res<Forms>>),
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
    (items, doors, containers, stashes): (
        Query<(&GlobalTransform, &Lying, Option<&TextName>)>,
        Query<&GlobalTransform, With<Door>>,
        Query<(Entity, &GlobalTransform, &crate::containers::Container)>,
        Query<(&GlobalTransform, &crate::stashes::Stash)>,
    ),
    (heights, buildings): (Option<Res<Heights>>, Option<Res<Buildings>>),
    heroes: Query<(
        Option<&Holding>,
        &Transform,
        &Body,
        Option<&crate::effects::Effects>,
    )>,
    (selected, arts, pack, mut glow): (
        Res<crate::making::Selected>,
        Option<Res<crate::arts::HeroArts>>,
        Option<Res<crate::pickup::Pack>>,
        ResMut<Glow>,
    ),
) {
    let in_game = state.is_some_and(|state| *state.get() == AppState::InGame);
    let over_interface = hovered.is_some_and(|hovered| {
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
    let look = match ray {
        Some(ray) if in_game && window.is_none() && !over_interface => {
            let near = |at: &GlobalTransform| {
                let distance = attack::from_line(ray, at.translation());
                (distance <= attack::PICKED_WITHIN).then_some(distance)
            };
            let nearest = |found: &mut dyn Iterator<Item = (f32, Look)>| {
                found
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, look)| look)
            };
            // A foe as `attack::aim` takes one: alive, hostile and of no dialogue.
            let is_foe = |mind: &Mind, talk: Option<&Talk>| talk.is_none() && mind.hostile();
            let alive = || {
                creatures
                    .iter()
                    .filter(|(_, _, _, health, ..)| health.current > 0)
            };
            let by_night = forms.as_deref().is_some_and(Forms::by_night);
            let hero = heroes.iter().next();
            let holding = hero.and_then(|(holding, ..)| holding);
            foe_under = alive()
                .filter(|(_, _, mind, _, talk, _)| is_foe(mind, *talk))
                .filter_map(|(entity, at, ..)| Some((near(at)?, entity)))
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, entity)| entity);
            let foe = foe_under.map(|_| foe_look(holding.map(|holding| &holding.0), by_night));
            foe.or_else(|| {
                nearest(&mut alive().filter_map(|(_, at, mind, _, talk, trades)| {
                    let distance = near(at)?;
                    if is_foe(mind, talk) {
                        return None;
                    }
                    let dialogue = crate::talk::dialogue_of(talk, mind, npcs.as_deref()).is_some();
                    let look = creature_look(trades.map_or(0, |trades| trades.0), dialogue)?;
                    Some((distance, look))
                }))
            })
            .or_else(|| {
                let (_, kind, text) = items
                    .iter()
                    .filter_map(|(at, lying, text)| Some((near(at)?, lying.dropped.clone(), text)))
                    .min_by(|a, b| a.0.total_cmp(&b.0))?;
                item = Some((Some(kind), text.map(|text| text.0.clone())));
                Some(Look::Hand)
            })
            .or_else(|| nearest(&mut doors.iter().filter_map(|at| Some((near(at)?, Look::Hand)))))
            // A container that opens, not one emptied (`0x612843`, #1380).
            .or_else(|| {
                let (_, entity) = containers
                    .iter()
                    .filter(|(_, at, container)| container.opens() && near(at).is_some())
                    .filter_map(|(entity, at, _)| Some((near(at)?, entity)))
                    .min_by(|a, b| a.0.total_cmp(&b.0))?;
                container_under = Some(entity);
                Some(Look::Hand)
            })
            // A hidden stash: the hand and its text (`0x612909`, #1518).
            .or_else(|| {
                let (_, stash) = stashes
                    .iter()
                    .filter_map(|(at, stash)| {
                        Some((crate::stashes::picked(ray, at.translation())?, stash))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))?;
                item = Some((None, Some(stash.text.clone())));
                Some(Look::Hand)
            })
            .or_else(|| {
                let (heights, buildings) = (heights.as_deref()?, buildings.as_deref()?);
                let (_, transform, body, _) = hero?;
                let height = transform.translation.y;
                let picked = pick::pick(ray, heights, buildings, (height, body.level()))?;
                let cell = paths::cell_of(picked.point.x, picked.point.z);
                heights.on_stairs(cell, picked.level).then_some(Look::Hand)
            })
            .unwrap_or_default()
        }
        _ => Look::Arrow,
    };
    // The world map's pin's mode shows the pin (`0x6C8140`).
    let look = if map.is_some_and(|map| map.pin_mode()) {
        Look::Pushpin
    } else {
        look
    };
    shown.set_if_neq(Shown(look));
    named.set_if_neq(Named(item.filter(|_| look == Look::Hand)));
    foe_shown.set_if_neq(Foe(foe_under));
    highlit.set_if_neq(Highlit(foe_under.or(container_under)));
    let in_world = ray.is_some() && in_game && window.is_none() && !over_interface;
    let glows = in_world
        && pack.as_ref().is_none_or(|pack| pack.held.is_none())
        && selected.0.zip(arts.as_deref()).is_some_and(|(art, arts)| {
            let effects = heroes.iter().next().and_then(|(.., effects)| effects);
            // The art of the hero's form for the selected one (`0x552370`).
            let art = opensacred_rules::arts::right_button(art, arts.kind, |key| {
                effects.is_some_and(|effects| effects.holds_any(&[key]))
            });
            crate::making::may_make(arts, art, effects).is_ok()
                && admits(crate::making::launch(arts, art), foe_under.is_some())
        });
    glow.set_if_neq(Glow(glows));
}

/// The item's name beside the cursor while the hand is over it (`0x611E70`, as it draws): its
/// text of the game's texts by the name the scripts gave it, else by its type ([`Named::shown`]),
/// white, 24 of the original's pixels right of the cursor and 16 down; none without a name.
fn name_tag(
    mut commands: Commands,
    named: Res<Named>,
    pointer: Res<Pointer>,
    ui_scale: Res<UiScale>,
    (texts, tables, assets): (Option<Res<Texts>>, Option<Res<Tables>>, Res<AssetServer>),
    mut tags: Query<(Entity, &mut Node, &mut Text), With<NameTag>>,
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
    // The window's pixels in the interface's, which `UiScale` scales (`hud::scale`).
    let at = at / ui_scale.0 + NAME_OFFSET;
    if let Ok((_, mut node, mut text)) = tags.single_mut() {
        node.left = Val::Px(at.x);
        node.top = Val::Px(at.y);
        if text.0 != name {
            name.clone_into(&mut text.0);
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
        TextColor(Color::WHITE),
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
            .add_systems(Update, (choose, name_tag).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_glows_over_what_the_arts_launch_admits() {
        assert!(admits(Some(1), true));
        assert!(!admits(Some(1), false));
        assert!(admits(Some(3), false) && admits(Some(5), false) && admits(Some(0), false));
        assert!(!admits(Some(2), true) && !admits(Some(4), true) && !admits(None, true));
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
        use opensacred_data::text::message;
        let texts = Texts::of(&format!(
            "{} = a type's name\n{} = a quest's name\n{} = {{\"\"}}\n",
            message("6001"),
            message("QUEST_PLANT"),
            message("EMPTY")
        ));
        let named = |item, text: Option<&str>| {
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
            };
            Named(Some((Some(dropped), text.map(str::to_owned))))
        };
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
