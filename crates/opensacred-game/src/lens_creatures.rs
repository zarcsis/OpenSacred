//! The creatures in the lens of the world map and of the overview (#1571; docs/re/maps.md, "The
//! lens's creatures"; `0x6CEAB9` to `0x6CF10E`): each living or dead creature of the lens's
//! sectors marked by a dot of 6 × 6 (`WORLDMAP_WAYPOINT`) in a colour by what it is to the hero,
//! a merchant by its cursor's picture in yellow in a quad of 48 × 48, at its iso point over 12
//! from the lens's point; within the hero's sight on both axes, beyond it on an explored cell
//! unless red, a merchant anywhere once its area's region has run its init.
//!
//! Not followed: the ridden horses' colours, the creatures' floors' lift, the interiors' sight of
//! 300, the sight's bonuses of the modifier 853 and of the sets, the flags `+0x200` `0x100` and
//! `0x200` among the talkable, the dynamic quests' cyan and the debug toggle's animals.

use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::bar::UiImages;
use crate::tooltip::argb;

/// The dot's picture and side, a merchant's quad's side, and the colours (`0x6CEAB9`): blue by
/// default, the heroes' class's, a follower of the hero's, the dead's grey, the talkable's white,
/// the hostile's red and the merchants' yellow.
pub(crate) const DOT: &str = "worldmap_waypoint";
const DOT_SIDE: f32 = 6.0;
const MERCHANT_SIDE: f32 = 48.0;
const BLUE: u32 = 0xFF00_00FF;
const HERO: u32 = 0xFFFF_E400;
const FOLLOWER: u32 = 0xFFBF_B400;
const DEAD: u32 = 0xFFA0_A0A0;
const TALKS: u32 = 0xFFFF_FFFF;
const HOSTILE: u32 = 0xFFFF_0000;
const MERCHANT: u32 = 0xFFFF_FF00;
/// The creature records' classes: the heroes' and the animals' (docs/re/fight.md).
const HEROES: u8 = 1;
const ANIMALS: u8 = 6;
/// The children's types (`0x43F670`) and the waters' crabs and fish, which the lens skips
/// (`0x434070`, the category `0x20`).
const CHILDREN: [u32; 7] = [229, 230, 231, 702, 703, 704, 705];
const WATER: std::ops::RangeInclusive<u32> = 163..=166;
/// The dialogue's icons below this are its "!" and "?", not a townsperson's activity
/// (`0x54FBC0`).
const TALK_ICONS: i32 = 50;
/// The hero's sight in world units by day and at night (`0x6CEAB9`).
const SIGHT: f32 = 650.0;
const NIGHT_SIGHT: f32 = 350.0;
/// The merchants' trades, in the order the lens tests them, and their pictures' kinds
/// (`world_map::PEOPLE`: the trader, the smith, the combat master, the horse trader).
const TRADES: [(u32, usize); 4] = [
    (opensacred_rules::quests::MERCHANT, 0),
    (opensacred_rules::quests::WIZARD, 2),
    (opensacred_rules::quests::SMITH, 1),
    (opensacred_rules::quests::TRADER, 3),
];

/// What a creature is to the lens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Kind {
    pub class: u8,
    pub creature: u32,
    pub follows_hero: bool,
    pub alive: bool,
    pub talks: bool,
    pub hostile: bool,
    pub trades: u32,
}

/// A creature's mark in the lens: a dot of a colour, or a merchant's picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    Dot(u32),
    Merchant(usize),
}

impl Mark {
    /// Whether it is red, which beyond the sight hides it.
    fn red(self) -> bool {
        self == Self::Dot(HOSTILE)
    }
}

/// The mark of a creature (`0x6CEAB9`), none for the waters' creatures and the animals.
pub(crate) fn mark(kind: Kind) -> Option<Mark> {
    if WATER.contains(&kind.creature) {
        return None;
    }
    if kind.class == HEROES {
        return Some(Mark::Dot(HERO));
    }
    if CHILDREN.contains(&kind.creature) {
        return Some(Mark::Dot(BLUE));
    }
    if kind.follows_hero {
        return Some(Mark::Dot(FOLLOWER));
    }
    if !kind.alive {
        return Some(Mark::Dot(DEAD));
    }
    if kind.talks {
        return Some(Mark::Dot(TALKS));
    }
    if kind.hostile {
        return Some(Mark::Dot(HOSTILE));
    }
    if let Some(&(_, picture)) = TRADES.iter().find(|(bit, _)| kind.trades & bit != 0) {
        return Some(Mark::Merchant(picture));
    }
    (kind.class != ANIMALS).then_some(Mark::Dot(BLUE))
}

/// Whether a mark shows (`0x6CEAB9`): within the sight on both axes, beyond it on an explored
/// cell unless red, or a merchant in an area whose region has run its init.
pub(crate) fn shows(mark: Mark, within: bool, explored: bool, merchant_area: bool) -> bool {
    within || (explored && !mark.red()) || (matches!(mark, Mark::Merchant(_)) && merchant_area)
}

/// What the lens reads of a creature.
type Seen = (
    Entity,
    &'static GlobalTransform,
    &'static crate::ai::Mind,
    Option<&'static crate::fight::Health>,
    Option<&'static crate::npcs::Dies>,
    Option<&'static crate::icons::Icon>,
    Option<&'static crate::npcs::Trades>,
    Option<&'static crate::followers::Follower>,
);

/// The world's creatures as the lens reads them.
#[derive(SystemParam)]
pub(crate) struct Creatures<'w, 's> {
    creatures: Query<'w, 's, Seen>,
    heroes: Query<'w, 's, (Entity, &'static GlobalTransform), With<crate::hero::Body>>,
    clock: Option<Res<'w, crate::daynight::Clock>>,
    land: Res<'w, crate::explored::Land>,
    streamer: Option<Res<'w, crate::sectors::Streamer>>,
    live: Option<NonSend<'w, crate::scripts::Live>>,
    data: Option<Res<'w, crate::game::Data>>,
}

impl Creatures<'_, '_> {
    /// The marks and world points of the creatures within `reach` world units of `at` on both
    /// axes, the hero's own first.
    pub(crate) fn marks(
        &self,
        at: Vec2,
        reach: f32,
        classes: &mut HashMap<u32, u8>,
    ) -> Vec<(Vec2, Mark)> {
        if classes.is_empty()
            && let Some(creatures) = self
                .data
                .as_deref()
                .and_then(|data| data.0.tables.creatures().ok())
        {
            classes.extend(
                creatures
                    .iter()
                    .map(|creature| (creature.id, creature.class)),
            );
        }
        let mut marks = Vec::new();
        let Some((hero_entity, hero)) = self.heroes.iter().next() else {
            return marks;
        };
        let hero = hero.translation().xz();
        marks.push((hero, Mark::Dot(HERO)));
        let night = self
            .clock
            .as_deref()
            .is_some_and(|clock| clock.phase() != crate::daynight::Phase::Day);
        let sight = if night { NIGHT_SIGHT } else { SIGHT };
        for (entity, place, mind, health, dies, icon, trades, follower) in &self.creatures {
            let point = place.translation().xz();
            if (point - at).abs().max_element() > reach {
                continue;
            }
            let kind = Kind {
                class: classes.get(&mind.creature).copied().unwrap_or(0),
                creature: mind.creature,
                follows_hero: follower.is_some_and(|follower| follower.master == hero_entity),
                alive: dies.is_none() && health.is_none_or(|health| health.current > 0),
                talks: icon.is_some_and(|icon| icon.value() < TALK_ICONS),
                hostile: mind.hostile(),
                trades: trades.map_or(0, |trades| trades.0),
            };
            let Some(mark) = mark(kind) else {
                continue;
            };
            let within = (point - hero).abs().max_element() <= sight;
            let cell = crate::paths::cell_of(point.x, point.y);
            let explored = self.land.explored(cell);
            let merchant_area = self
                .streamer
                .as_deref()
                .zip(self.live.as_deref())
                .and_then(|(streamer, live)| {
                    let sector = (
                        u32::try_from(cell.0 / 64).ok()?,
                        u32::try_from(cell.1 / 64).ok()?,
                    );
                    Some(live.region_ran(streamer.area(sector)?))
                })
                .unwrap_or(false);
            if entity != hero_entity && shows(mark, within, explored, merchant_area) {
                marks.push((point, mark));
            }
        }
        marks
    }
}

/// Spawns the marks `marks` as the children of `parent`, a plane whose screen point
/// (512, 384) shows the iso point `point`.
pub(crate) fn spawn(
    commands: &mut Commands,
    parent: Entity,
    (images, assets): (&UiImages, &AssetServer),
    marks: &[(Vec2, Mark)],
    point: Vec2,
) {
    let dot = images.file(DOT).map(str::to_owned);
    for &(at, mark) in marks {
        let place = (crate::minimap::iso(at) - point.trunc()) / 12.0 + Vec2::new(512.0, 384.0);
        let (file, side, colour) = match mark {
            Mark::Dot(colour) => (dot.clone(), DOT_SIDE, colour),
            Mark::Merchant(kind) => (
                crate::world_map::PEOPLE
                    .get(kind)
                    .and_then(|name| images.file(name))
                    .map(str::to_owned),
                MERCHANT_SIDE,
                MERCHANT,
            ),
        };
        let Some(file) = file else {
            continue;
        };
        let child = commands
            .spawn((
                ImageNode::new(crate::hud::image(assets, file)).with_color(argb(colour)),
                crate::bar::placed(place - Vec2::splat(side / 2.0), Vec2::splat(side)),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(parent).add_child(child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_creatures_mark_follows_the_lens_order() {
        let townsperson = Kind {
            class: 2,
            creature: 644,
            alive: true,
            ..Kind::default()
        };
        assert_eq!(mark(townsperson), Some(Mark::Dot(BLUE)));
        assert_eq!(
            mark(Kind {
                class: HEROES,
                ..townsperson
            }),
            Some(Mark::Dot(HERO))
        );
        let wolf = Kind {
            class: ANIMALS,
            hostile: true,
            ..townsperson
        };
        assert_eq!(mark(wolf), Some(Mark::Dot(HOSTILE)));
        assert_eq!(
            mark(Kind {
                alive: false,
                ..wolf
            }),
            Some(Mark::Dot(DEAD))
        );
        assert_eq!(
            mark(Kind {
                hostile: false,
                ..wolf
            }),
            None
        );
        assert_eq!(
            mark(Kind {
                trades: opensacred_rules::quests::TRADER,
                ..townsperson
            }),
            Some(Mark::Merchant(3))
        );
        assert_eq!(
            mark(Kind {
                talks: true,
                trades: opensacred_rules::quests::MERCHANT,
                ..townsperson
            }),
            Some(Mark::Dot(TALKS))
        );
        assert_eq!(
            mark(Kind {
                creature: 703,
                ..wolf
            }),
            Some(Mark::Dot(BLUE))
        );
        assert_eq!(
            mark(Kind {
                creature: 164,
                ..townsperson
            }),
            None
        );
        // Beyond the sight: a red one hides, a merchant shows in an area initialised.
        assert!(!shows(Mark::Dot(HOSTILE), false, true, true));
        assert!(shows(Mark::Dot(BLUE), false, true, false));
        assert!(shows(Mark::Merchant(0), false, false, true));
        assert!(!shows(Mark::Dot(BLUE), false, false, true));
    }
}
