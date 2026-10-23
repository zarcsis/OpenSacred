//! The commands for testing that read the world (docs/10 §6, #521): the creatures about the hero
//! and a creature's fight values, as the original's driver prints them (`creatures`, `stats`,
//! docs/03 §3.8), so that a check reads the same lines from both games.

use bevy::prelude::*;

use crate::fight::{Combat, Health, Ranked};
use crate::hero::Body;
use crate::npcs::Dies;
use crate::terrain::{Heights, Level};
use crate::{ai, crowd, fight, paths};

/// The kinds of damage, as the lines name them (docs/re/fight.md).
const KINDS: [&str; 4] = ["physical", "fire", "magic", "poison"];

/// A creature as `creatures` lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    pub id: Entity,
    pub creature: u32,
    pub cell: (i32, i32),
    /// The building's floor it stands on, else its inside layer, if it stands inside.
    pub inside: Option<String>,
    pub health: Option<(u32, u32)>,
    pub level: u32,
    pub hostile: bool,
    pub awake: bool,
    pub fighting: bool,
    pub rank: fight::Rank,
    pub dying: bool,
    /// Whether it has a dialogue, which holds it (`+0x245`, `minds::Mind::dialogue`).
    pub dialogue: bool,
}

impl Listed {
    /// Its line: as the original's driver prints a creature, then what the game adds.
    pub fn line(&self) -> String {
        let (x, y) = self.cell;
        let mut line = format!("{} type {} cell {x},{y}", self.id, self.creature);
        if let Some(inside) = &self.inside {
            line.push_str(&format!(" {inside}"));
        }
        if let Some((current, most)) = self.health {
            line.push_str(&format!(" health {current}/{most}"));
        }
        line.push_str(&format!(" level {}", self.level));
        line.push_str(if self.hostile {
            " hostile"
        } else {
            " peaceful"
        });
        for (holds, what) in [
            (self.awake, " awake"),
            (self.fighting, " fighting"),
            (self.rank == fight::Rank::Champion, " champion"),
            (self.rank == fight::Rank::Boss, " boss"),
            (self.dying, " dying"),
            (self.dialogue, " dialogue"),
        ] {
            if holds {
                line.push_str(what);
            }
        }
        line
    }
}

/// Where a creature on `level` stands, as the lines name it: `floor F` of a building, else its
/// inside layer; `None` on the ground.
fn inside(heights: Option<&Heights>, level: Level) -> Option<String> {
    level?;
    Some(match heights.and_then(|heights| heights.floor(level)) {
        Some((_, floor)) => format!("floor {floor}"),
        None => crate::overlay::place(level),
    })
}

/// The hero's world point.
fn hero(world: &mut World) -> Result<Vec3, String> {
    world
        .query_filtered::<&Transform, With<Body>>()
        .iter(world)
        .next()
        .map(|transform| transform.translation)
        .ok_or_else(|| "the hero is not in the world yet".into())
}

/// The creatures within `radius` cells of the hero along each axis, the nearest first, ties by
/// their ids.
pub fn creatures(world: &mut World, radius: u32) -> Result<Vec<String>, String> {
    let at = hero(world)?;
    let home = paths::cell_of(at.x, at.z);
    let radius = i32::try_from(radius).unwrap_or(i32::MAX);
    let mut query = world.query_filtered::<(
        Entity,
        &Transform,
        &ai::Mind,
        Option<&Health>,
        Option<&Ranked>,
        Option<&crowd::InTheWay>,
        Has<Dies>,
    ), Without<Body>>();
    let heights = world.get_resource::<Heights>();
    let mut found: Vec<(f32, Listed)> = query
        .iter(world)
        .filter_map(|(id, transform, mind, health, rank, way, dying)| {
            let here = transform.translation;
            let cell = paths::cell_of(here.x, here.z);
            if (cell.0 - home.0).abs() > radius || (cell.1 - home.1).abs() > radius {
                return None;
            }
            let listed = Listed {
                id,
                creature: mind.creature,
                cell,
                inside: inside(heights, way.and_then(|way| way.level)),
                health: health.map(|health| (health.current, health.most)),
                level: health.map_or(mind.level, |health| health.level),
                hostile: mind.hostile(),
                awake: mind.awake(),
                fighting: mind.fighting(),
                rank: rank.map_or_else(fight::Rank::default, |rank| rank.0),
                dying,
                dialogue: mind.dialogue,
            };
            Some((here.distance(at), listed))
        })
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.id.cmp(&b.1.id)));
    let mut lines = vec![format!(
        "{} creatures within {radius} cells of the hero",
        found.len()
    )];
    lines.extend(found.iter().map(|(_, listed)| listed.line()));
    Ok(lines)
}

/// A creature's fight values as `stats` prints them.
#[derive(Clone, Debug, PartialEq)]
pub struct Stats {
    /// Its id, `None` for the hero.
    pub id: Option<Entity>,
    pub creature: Option<u32>,
    pub health: Option<Health>,
    /// Its type's attributes (`Creature.pak`), where it keeps them: the hero's.
    pub attributes: Option<[u8; 6]>,
    pub combat: Option<Combat>,
    pub rank: fight::Rank,
}

impl Stats {
    /// Its lines: as the original's driver prints them where the game has the values, already
    /// multiplied by the attack record (docs/re/fight.md).
    pub fn lines(&self) -> Vec<String> {
        let numbers = |values: &[f32]| {
            values
                .iter()
                .map(|v| format!("{v:.1}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut head = match self.id {
            Some(id) => format!("{id}"),
            None => "the hero".into(),
        };
        if let Some(creature) = self.creature {
            head.push_str(&format!(" type {creature}"));
        }
        if let Some(health) = self.health {
            head.push_str(&format!(
                " level {} health {}/{}",
                health.level, health.current, health.most
            ));
        }
        match self.rank {
            fight::Rank::Common => {}
            fight::Rank::Champion => head.push_str(" champion"),
            fight::Rank::Boss => head.push_str(" boss"),
            // A champion's levelling alone, which the original's lists show as nothing (#621).
            fight::Rank::Levelled => head.push_str(" levelled"),
        }
        let mut lines = vec![head];
        if let Some(attributes) = self.attributes {
            let attributes: Vec<String> = attributes.iter().map(u8::to_string).collect();
            lines.push(format!("attributes (base) {}", attributes.join(", ")));
        }
        match &self.combat {
            Some(combat) => {
                lines.push(format!("A {:.1}, D {:.1}", combat.attack, combat.defence));
                let least: Vec<f32> = combat.damage.iter().map(|[least, _]| *least).collect();
                let most: Vec<f32> = combat.damage.iter().map(|[_, most]| *most).collect();
                lines.push(format!(
                    "damage ({}) least {}, most {}",
                    KINDS.join(", "),
                    numbers(&least),
                    numbers(&most)
                ));
                lines.push(format!("resistances {}", numbers(&combat.resistance)));
                lines.push(format!("attack speed {:.0}", combat.speed));
            }
            None => lines.push("no fight values".into()),
        }
        lines
    }
}

/// The fight values of creature `id`, the hero's by default.
pub fn stats(world: &mut World, id: Option<Entity>) -> Result<Vec<String>, String> {
    let stats = match id {
        None => {
            let mut query = world.query_filtered::<(
                Option<&Health>,
                Option<&Combat>,
                Option<&fight::Base>,
            ), With<Body>>();
            let (health, combat, base) = query
                .iter(world)
                .next()
                .ok_or("the hero is not in the world yet")?;
            Stats {
                id: None,
                creature: base.map(|base| base.id),
                health: health.copied(),
                attributes: base.map(|base| base.attributes),
                combat: combat.copied(),
                rank: fight::Rank::Common,
            }
        }
        Some(id) => {
            let mut query =
                world.query::<(&ai::Mind, Option<&Health>, Option<&Combat>, Option<&Ranked>)>();
            let (mind, health, combat, rank) = query
                .get(world, id)
                .map_err(|_| format!("{id} is no creature"))?;
            Stats {
                id: Some(id),
                creature: Some(mind.creature),
                health: health.copied(),
                attributes: None,
                combat: combat.copied(),
                rank: rank.map_or_else(fight::Rank::default, |rank| rank.0),
            }
        }
    };
    Ok(stats.lines())
}

/// Creature `id`'s health, the hero's by default, to its most or to `to` (`heal`), at most its
/// most.
pub fn heal(world: &mut World, id: Option<Entity>, to: Option<u32>) -> Result<String, String> {
    if to == Some(0) {
        return Err("--to takes 1 to the most health".into());
    }
    let mut health = match id {
        None => {
            let mut heroes = world.query_filtered::<&mut Health, With<Body>>();
            heroes
                .iter_mut(world)
                .next()
                .ok_or("the hero is not in the world yet")?
        }
        Some(id) => {
            let mut creatures = world.query_filtered::<&mut Health, Without<Dies>>();
            creatures
                .get_mut(world, id)
                .map_err(|_| format!("{id} is no living creature"))?
        }
    };
    health.current = to.map_or(health.most, |to| to.min(health.most));
    let line = format!("health {}/{}", health.current, health.most);
    Ok(match id {
        Some(id) => format!("{id} {line}"),
        None => line,
    })
}

/// How far about a cell, in cells along each axis, `spawn` looks for cells to stand creatures on.
const SPAWN_REACH: i32 = 16;

/// Up to `count` cells for creatures about `centre` on `level` (`spawn`): `centre` first where a
/// creature may step into it, then the nearest others ring by ring within [`SPAWN_REACH`], never
/// the hero's cell `hero`; `centre` alone where none may be stepped into.
pub fn cells_about(
    heights: &Heights,
    centre: (i32, i32),
    level: Level,
    count: usize,
    hero: (i32, i32),
) -> Vec<(i32, i32)> {
    let mut cells = Vec::new();
    'rings: for ring in 0..=SPAWN_REACH {
        for dy in -ring..=ring {
            for dx in -ring..=ring {
                if dx.abs().max(dy.abs()) != ring {
                    continue;
                }
                let cell = (centre.0 + dx, centre.1 + dy);
                if cell != hero && heights.walkable_on(cell, level) {
                    cells.push(cell);
                    if cells.len() == count {
                        break 'rings;
                    }
                }
            }
        }
    }
    if cells.is_empty() && count > 0 {
        cells.push(centre);
    }
    cells
}

/// The creature whose id the lines print as `text` (`5450v1`), or whose index it is (`5450`).
pub fn creature(world: &mut World, text: &str) -> Result<Entity, String> {
    let mut minds = world.query_filtered::<Entity, With<ai::Mind>>();
    minds
        .iter(world)
        .find(|id| {
            let shown = id.to_string();
            shown == text
                || shown
                    .split_once('v')
                    .is_some_and(|(index, _)| index == text)
        })
        .ok_or_else(|| format!("{text} is no creature"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combat() -> Combat {
        Combat(opensacred_rules::fight::Combat {
            attack: 31.0,
            defence: 25.5,
            damage: [[4.0, 9.0], [0.0, 0.0], [1.0, 2.5], [0.0, 0.0]],
            resistance: [3.0, 0.0, 0.0, 10.0],
            damping: [0; 4],
            speed: 140.0,
            pace: 130.0,
        })
    }

    #[test]
    fn a_creature_lists_as_the_originals_driver_prints_it() {
        let listed = Listed {
            id: Entity::from_raw_u32(5450).unwrap(),
            creature: 688,
            cell: (3253, 2867),
            inside: None,
            health: Some((83, 83)),
            level: 1,
            hostile: false,
            awake: false,
            fighting: false,
            rank: fight::Rank::Common,
            dying: false,
            dialogue: false,
        };
        assert_eq!(
            listed.line(),
            format!(
                "{} type 688 cell 3253,2867 health 83/83 level 1 peaceful",
                listed.id
            )
        );
        let fighter = Listed {
            inside: Some("floor 2".into()),
            hostile: true,
            awake: true,
            fighting: true,
            rank: fight::Rank::Champion,
            dialogue: true,
            ..listed.clone()
        };
        assert!(
            fighter
                .line()
                .ends_with("floor 2 health 83/83 level 1 hostile awake fighting champion dialogue"),
            "{}",
            fighter.line()
        );
    }

    #[test]
    fn stats_print_the_fight_values() {
        let stats = Stats {
            id: None,
            creature: Some(1),
            health: Some(Health {
                current: 100,
                most: 119,
                level: 1,
            }),
            attributes: Some([20, 12, 18, 10, 14, 9]),
            combat: Some(combat()),
            rank: fight::Rank::Common,
        };
        assert_eq!(
            stats.lines(),
            [
                "the hero type 1 level 1 health 100/119",
                "attributes (base) 20, 12, 18, 10, 14, 9",
                "A 31.0, D 25.5",
                "damage (physical, fire, magic, poison) least 4.0, 0.0, 1.0, 0.0, most 9.0, 0.0, \
                 2.5, 0.0",
                "resistances 3.0, 0.0, 0.0, 10.0",
                "attack speed 140",
            ]
        );
    }

    #[test]
    fn spawn_stands_creatures_about_a_cell_off_the_heros() {
        use opensacred_data::world::{Cell, SIDE, Sector};
        let cell = |ground| Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground,
        };
        let sector = |cells| Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: Vec::new(),
        };
        // Grass, water at (5, 5).
        let mut cells = vec![cell(0x40); SIDE * SIDE];
        cells[5 * SIDE + 5] = cell(0x92);
        let mut heights = Heights::default();
        heights.add(&sector(cells));
        // A walkable centre first, then its ring, row by row.
        assert_eq!(
            cells_about(&heights, (10, 10), None, 3, (0, 0)),
            [(10, 10), (9, 9), (10, 9)]
        );
        // About the water: its ring, but the hero's cell (4, 4).
        assert_eq!(
            cells_about(&heights, (5, 5), None, 3, (4, 4)),
            [(5, 4), (6, 4), (4, 5)]
        );
        // Nothing to stand on: the centre alone.
        let mut heights = Heights::default();
        heights.add(&sector(vec![cell(0x92); SIDE * SIDE]));
        assert_eq!(cells_about(&heights, (5, 5), None, 4, (0, 0)), [(5, 5)]);
    }

    #[test]
    fn an_id_finds_its_creature() {
        let mut world = World::new();
        world.spawn_empty();
        let mind = ai::Mind::new(Vec3::ZERO, 1, (false, false), None, [1.0, 2.0], (76, 0));
        let goblin = world.spawn(mind).id();
        assert_eq!(creature(&mut world, &goblin.to_string()), Ok(goblin));
        let index = goblin.to_string();
        let index = index.split_once('v').unwrap().0;
        assert_eq!(creature(&mut world, index), Ok(goblin));
        assert!(creature(&mut world, "999").is_err());
    }
}
