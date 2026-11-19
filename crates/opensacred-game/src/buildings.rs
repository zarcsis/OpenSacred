//! The buildings' states (docs/re/drawing.md, "Buildings: masks and layers"). A cell with flag
//! bit 0 belongs to a building, whose anchor cell its offsets (`+0x1C`, `+0x1D`) point to. The
//! client draws a building's parts (the statics of its cells flagged `0x8`, or of the types of
//! lists 0, 2 and 4, `statics`) only when their mask is the building's state, and with the
//! world's objects of layer 0, the ground's, which it draws on every cell, the objects of its
//! cells on the floor the state gives (the index of its lowest bit) when that is not 0. Outside,
//! the state is 1: the shell with its roof. The client sets it to 2 as the hero steps onto an
//! entrance of the building and goes onto its inside layer, and back to 1 as the hero steps
//! onto an exit and comes out onto the ground (`0x5FCCB0`, docs/re/movement.md): the ground
//! floor cut open with its furniture, and the objects of layer 1, while the hero walks inside.
//! On the stairs to an upper floor `n` it becomes `1 << n`: that floor's parts and objects.
//! A script's `SetTriggerState` of a cell gives the building there a state of its own
//! (`0x4968A0`, #2888): the land's banners turn, the rocks of a pass open, and a save keeps it.

use std::collections::HashMap;

use bevy::prelude::*;
use opensacred_data::world::{SIDE, Sector};
use serde::{Deserialize, Serialize};

use crate::frame::Step;
use crate::game::AppState;
use crate::hero::Body;
use crate::scripts::BuildingStates;
use crate::terrain::{Heights, Level};

/// The cell flag of a building's cells.
pub const BUILDING_CELL: u8 = 0x1;
/// The state of a building while the hero is outside: its shell's mask.
pub const OUTSIDE: u8 = 1;
/// The state of the building the hero is in.
pub const INSIDE: u8 = 2;

/// A building, by the global cell its parts' cells point to.
pub type Anchor = (u32, u32);

/// The global cell of cell `index` of `sector`.
pub fn cell(sector: &Sector, index: usize) -> Option<Anchor> {
    let side = u32::try_from(SIDE).ok()?;
    let column = u32::try_from(index % SIDE).ok()?;
    let row = u32::try_from(index / SIDE).ok()?;
    Some((sector.x * side + column, sector.y * side + row))
}

/// The building that cell `index` of `sector` belongs to, if it belongs to one.
pub fn anchor(sector: &Sector, index: usize) -> Option<Anchor> {
    let cell = sector.cells.get(index)?;
    if cell.flags & BUILDING_CELL == 0 {
        return None;
    }
    let side = i64::try_from(SIDE).ok()?;
    let column = i64::try_from(index % SIDE).ok()? + i64::from(sector.x) * side;
    let row = i64::try_from(index / SIDE).ok()? + i64::from(sector.y) * side;
    let x = u32::try_from(column + i64::from(cell.offsets[0])).ok()?;
    let y = u32::try_from(row + i64::from(cell.offsets[1])).ok()?;
    Some((x, y))
}

/// The layer of a building's world objects that a state shows: the index of its lowest bit.
pub fn layer(state: u8) -> i32 {
    i32::try_from(state.trailing_zeros()).unwrap_or(0)
}

/// Statics of a building's parts that its state chooses: shown while the state is `mask`.
#[derive(Component)]
pub struct Part {
    pub building: Anchor,
    pub mask: u8,
}

/// The layer that a start script's object or NPC lies in, and the building it lies in it of:
/// layer 0 on the ground, of the building whose cell it stands on, if any; layer `n` on a
/// building's floor `n`. The client draws every cell with the world's objects of layer 0, and a
/// building's cells with those of the layer that the building's state shows too, when that is
/// not 0, or of all its floors when the building is open to the sky (`0x62AE90`,
/// docs/re/drawing.md): it shows on the ground always, on an open building's floor always, and
/// on another's floor while that is the state's layer.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layered {
    pub building: Option<Anchor>,
    pub layer: i32,
    /// Whether the building is open to the sky ([`Heights::open`]).
    pub open: bool,
}

impl Layered {
    /// An object at world `(x, z)` on `level`: on a building's floor, that floor; on an inside
    /// layer of no known building's, floor 1 of the building of its cell; on the ground, layer 0.
    pub fn at(heights: &Heights, x: f32, z: f32, level: Level) -> Self {
        let ground = heights.building(x, z).map(|(building, _)| building);
        let (building, layer) = match (level, heights.floor(level)) {
            (_, Some((building, floor))) => (Some(building), i32::from(floor)),
            (Some(_), None) => (ground, 1),
            (None, None) => (ground, 0),
        };
        Self::of(heights, building, layer)
    }

    /// An object in `layer` of `building`, open to the sky or not as the building is.
    pub fn of(heights: &Heights, building: Option<Anchor>, layer: i32) -> Self {
        Self {
            building,
            layer,
            open: building.is_some_and(|building| heights.open(building)),
        }
    }

    /// Whether it shows while the buildings are in their states: on the ground, on a building's
    /// cells too, whatever its state, and on an open building's floors; never above the ground
    /// where it is in no building.
    pub fn shown(&self, buildings: &Buildings) -> bool {
        match (self.layer, self.building) {
            (0, _) => true,
            (_, Some(_)) if self.open => true,
            (floor, Some(building)) => floor == layer(buildings.state(building)),
            (_, None) => false,
        }
    }

    /// Its visibility while the buildings are in their states.
    pub fn visibility(&self, buildings: &Buildings) -> Visibility {
        visibility(self.shown(buildings))
    }
}

fn visibility(shown: bool) -> Visibility {
    if shown {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

/// The buildings' states, each the word of its trigger's record (docs/re/drawing.md, "Buildings:
/// masks and layers"), [`OUTSIDE`] but for those kept here. The hero's steps set the state of
/// the building it goes into and of the one it comes out of (`0x5FCCB0`, through
/// `cTrigger::resetState` and `cTrigger::setState`), and a script's `SetTriggerState` of a cell
/// that of the building there (`0x4968A0`): each change stands until the next.
#[derive(Resource, Default)]
pub struct Buildings {
    /// The building the hero is in and its floor, from 1.
    entered: Option<(Anchor, u8)>,
    /// The states other than [`OUTSIDE`].
    states: HashMap<Anchor, u8>,
    /// The states that the scripts gave by a cell of a sector not loaded, by that cell, which its
    /// building takes as the sector comes.
    waiting: HashMap<(i32, i32), u8>,
    /// Whether a state changed since the parts and the objects were last shown.
    changed: bool,
}

/// The buildings' states as a save keeps them, as the original's keeps its triggers' words
/// (#2888): those other than [`OUTSIDE`] by building, and the scripts' states that wait for their
/// sectors by cell.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct SavedBuildings {
    pub states: Vec<(Anchor, u8)>,
    pub waiting: Vec<((i32, i32), u8)>,
}

impl Buildings {
    /// A building's state.
    pub fn state(&self, building: Anchor) -> u8 {
        self.states.get(&building).copied().unwrap_or(OUTSIDE)
    }

    /// Gives a building a state.
    fn set(&mut self, building: Anchor, state: u8) {
        let was = if state == OUTSIDE {
            self.states.remove(&building)
        } else {
            self.states.insert(building, state)
        };
        self.changed |= was.unwrap_or(OUTSIDE) != state;
    }

    /// The hero goes into the building `entered` onto its floor, from 1, or out onto the ground:
    /// the building it comes out of takes the state [`OUTSIDE`], an exit's, and the one it goes
    /// into its floor's bit, an entrance's or a stair's (`1 << floor`, [`INSIDE`] on the ground
    /// floor). Nothing changes while it stays where it was.
    fn walk(&mut self, entered: Option<(Anchor, u8)>) {
        if entered == self.entered {
            return;
        }
        if let Some((left, _)) = self.entered {
            self.set(left, OUTSIDE);
        }
        if let Some((building, floor)) = entered {
            self.set(
                building,
                1u8.checked_shl(u32::from(floor)).unwrap_or(INSIDE),
            );
        }
        self.entered = entered;
    }

    /// A script's state for the building that global `cell` belongs to: given at once in a
    /// loaded sector, none where the cell is no building's (`0x6354D0` finds none), else kept
    /// until the cell's sector comes.
    fn script(&mut self, heights: &Heights, cell: (i32, i32), state: u8) {
        if !heights.holds(cell) {
            self.waiting.insert(cell, state);
        } else if let Some(building) = heights.building_at(cell) {
            self.set(building, state);
        }
    }

    /// The scripts' states whose cells' sectors came, given their buildings.
    fn arrive(&mut self, heights: &Heights) {
        let came: Vec<(i32, i32)> = self
            .waiting
            .keys()
            .copied()
            .filter(|&cell| heights.holds(cell))
            .collect();
        for cell in came {
            if let Some(state) = self.waiting.remove(&cell) {
                self.script(heights, cell, state);
            }
        }
    }

    /// The states to save, in order.
    pub fn saved(&self) -> SavedBuildings {
        let mut states: Vec<_> = self.states.iter().map(|(&b, &s)| (b, s)).collect();
        let mut waiting: Vec<_> = self.waiting.iter().map(|(&c, &s)| (c, s)).collect();
        states.sort_unstable();
        waiting.sort_unstable();
        SavedBuildings { states, waiting }
    }

    /// A loaded save's states again, the building the hero stands in keeping its floor's bit.
    pub fn again(&mut self, saved: SavedBuildings) {
        self.states = saved.states.into_iter().collect();
        self.waiting = saved.waiting.into_iter().collect();
        if let Some((building, floor)) = self.entered {
            self.states.insert(
                building,
                1u8.checked_shl(u32::from(floor)).unwrap_or(INSIDE),
            );
        }
        self.states.retain(|_, state| *state != OUTSIDE);
        self.changed = true;
    }
}

/// Gives the buildings the states that the scripts gave by a cell, and those that waited for
/// their sectors as the sectors come.
fn scripted(
    mut buildings: ResMut<Buildings>,
    heights: Res<Heights>,
    mut given: ResMut<BuildingStates>,
) {
    for (cell, word) in given.0.drain(..) {
        match u8::try_from(word) {
            Ok(state) => buildings.script(&heights, cell, state),
            Err(_) => warn!("buildings: a script's state {word:#x} at {cell:?} is no building's"),
        }
    }
    if heights.is_changed() && !buildings.waiting.is_empty() {
        buildings.arrive(&heights);
    }
}

/// Finds the building the hero is in and shows each building's parts and objects of its state,
/// when a state changes, as they come and as the objects change layers. The hero is in a
/// building while it walks on an inside layer: in the one whose cell it went onto the layer at,
/// its entrance or, at a start inside, the cell it starts on.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
pub fn enter(
    mut buildings: ResMut<Buildings>,
    heights: Res<Heights>,
    bodies: Query<(&Transform, &Body)>,
    mut shown: ParamSet<(
        Query<(&Part, &mut Visibility)>,
        Query<(&Part, &mut Visibility), Added<Part>>,
        Query<(&Layered, &mut Visibility)>,
        Query<(&Layered, &mut Visibility), Changed<Layered>>,
    )>,
) {
    // The building and the floor of the hero's level, or for a layer of no known building's,
    // the one whose cell it went onto the layer at, on its floor 1.
    let entered = bodies.iter().next().and_then(|(hero, body)| {
        let level = body.level()?;
        heights.floor(Some(level)).or_else(|| {
            buildings.entered.or_else(|| {
                heights
                    .building(hero.translation.x, hero.translation.z)
                    .map(|(building, _)| (building, 1))
            })
        })
    });
    buildings.walk(entered);
    let changed = std::mem::take(&mut buildings.changed);
    let buildings = &*buildings;
    let part = |part: &Part, mut shown: Mut<Visibility>| {
        shown.set_if_neq(visibility(part.mask == buildings.state(part.building)));
    };
    let object = |layered: &Layered, mut shown: Mut<Visibility>| {
        shown.set_if_neq(layered.visibility(buildings));
    };
    // All of them when a building's state changes, else those that just came or changed
    // layers.
    if changed {
        for (p, visibility) in &mut shown.p0() {
            part(p, visibility);
        }
        for (l, visibility) in &mut shown.p2() {
            object(l, visibility);
        }
    } else {
        for (p, visibility) in &mut shown.p1() {
            part(p, visibility);
        }
        for (l, visibility) in &mut shown.p3() {
            object(l, visibility);
        }
    }
}

/// Forgets the buildings' states of the game left.
fn leave(mut buildings: ResMut<Buildings>) {
    *buildings = Buildings::default();
}

/// The buildings' states, by the building the hero is in and by the scripts.
pub struct BuildingsPlugin;

impl Plugin for BuildingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Buildings>()
            .add_systems(Update, (scripted, enter).chain().in_set(Step::Buildings))
            .add_systems(OnExit(AppState::InGame), leave);
    }
}

#[cfg(test)]
mod tests {
    use opensacred_data::world::{Cell, Layer, LayerCell};

    use super::*;

    /// The buildings with the hero in `building` on `floor`.
    fn inside(building: Anchor, floor: u8) -> Buildings {
        let mut buildings = Buildings::default();
        buildings.walk(Some((building, floor)));
        buildings
    }

    #[test]
    fn a_buildings_cells_point_to_its_anchor() {
        let cell = |flags, offsets| Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets,
            flags,
            ground: 0,
        };
        let mut cells = vec![cell(0, [0, 0]); SIDE * SIDE];
        // Cell (column 24, row 17) of sector (50, 39) points 5 back and 2 up.
        cells[17 * SIDE + 24] = cell(0x3, [-5, -2]);
        cells[17 * SIDE + 25] = cell(0x0, [-5, -2]);
        let sector = Sector {
            x: 50,
            y: 39,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: Vec::new(),
        };
        let side = u32::try_from(SIDE).unwrap();
        assert_eq!(
            anchor(&sector, 17 * SIDE + 24),
            Some((50 * side + 19, 39 * side + 15))
        );
        assert_eq!(anchor(&sector, 17 * SIDE + 25), None);
        assert_eq!((layer(1), layer(2), layer(4), layer(16)), (0, 1, 2, 4));
    }

    #[test]
    fn a_buildings_state_is_its_floors_bit() {
        let state = |floor| {
            let buildings = inside((1, 1), floor);
            (buildings.state((1, 1)), buildings.state((2, 2)))
        };
        // Floor 1 the ground floor inside, state 2; up the stairs 4 and 8; the others outside.
        assert_eq!(state(1), (INSIDE, OUTSIDE));
        assert_eq!(state(2), (4, OUTSIDE));
        assert_eq!(state(3), (8, OUTSIDE));
        assert_eq!(Buildings::default().state((1, 1)), OUTSIDE);
    }

    #[test]
    fn an_object_shows_with_the_layer_of_its_buildings_state() {
        let cell = |flags| Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0, 0],
            flags,
            ground: 0x40,
        };
        // Cell (5, 5) of sector (0, 0) a building's, its own anchor, with an inside layer over
        // it, of no known building's floors.
        let mut cells = vec![cell(0); SIDE * SIDE];
        cells[5 * SIDE + 5] = cell(BUILDING_CELL);
        let sector = Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: Vec::new(),
            layers: vec![Layer {
                x: 5,
                y: 5,
                width: 1,
                height: 1,
                cells: vec![LayerCell {
                    heights2: [0; 4],
                    offsets: [0; 2],
                    flags: 0,
                    ground: 0x40,
                }],
            }],
            buildings: Vec::new(),
        };
        let mut heights = Heights::default();
        heights.add(&sector);
        let chapel = (5, 5);
        let at = |cell: f32, level| {
            let point = crate::iso::ground(cell + 0.5, cell + 0.5);
            Layered::at(&heights, point.x, point.z, level)
        };
        let on = |building, layer| Layered {
            building,
            layer,
            open: false,
        };
        // In the building, through its door: its layer 1; on its cell of the ground, its stairs,
        // layer 0; elsewhere, the ground's.
        let novice = at(5.0, Some((0, 0, 1)));
        let stairs = at(5.0, None);
        assert_eq!((novice, stairs), (on(Some(chapel), 1), on(Some(chapel), 0)));
        assert_eq!(at(2.0, None), on(None, 0));
        // The hero outside, inside, upstairs: layer 1 shows inside alone, layer 0 always, on the
        // building's cells too (#148).
        let outside = Buildings::default();
        let within = inside(chapel, 1);
        let upstairs = inside(chapel, 2);
        let shown = |object: Layered| {
            [&outside, &within, &upstairs].map(|buildings| object.shown(buildings))
        };
        assert_eq!(shown(novice), [false, true, false]);
        assert_eq!(shown(stairs), [true; 3]);
        // Off the buildings, the ground's shows always and a layer above it never.
        assert_eq!(shown(on(None, 0)), [true; 3]);
        assert_eq!(shown(on(None, 1)), [false; 3]);
    }

    #[test]
    fn an_open_buildings_floors_show_whatever_its_state() {
        // Two buildings of two floors, a castle's wall open to the sky and a closed tower.
        let building = |cell, open| opensacred_data::world::Building {
            cell,
            floors: vec![1, 2],
            heights: vec![0, 112],
            open,
        };
        let sector = Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells: Vec::new(),
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: vec![building(5, true), building(9, false)],
        };
        let mut heights = Heights::default();
        heights.add(&sector);
        let (wall, tower) = ((5, 0), (9, 0));
        assert!(heights.open(wall) && !heights.open(tower));
        let guard = |building| Layered::of(&heights, Some(building), 2);
        let outside = Buildings::default();
        let in_tower = inside(tower, 1);
        // A guard on the wall walk shows to a hero outside and in the tower (#203); one on the
        // tower's top only while the hero is up there.
        assert!(guard(wall).shown(&outside) && guard(wall).shown(&in_tower));
        assert!(!guard(tower).shown(&outside) && !guard(tower).shown(&in_tower));
        let on_top = inside(tower, 2);
        assert!(guard(tower).shown(&on_top));
    }

    #[test]
    fn the_heros_steps_set_the_states_of_the_buildings_it_goes_into_and_out_of() {
        let (house, tower) = ((1, 1), (9, 9));
        let mut buildings = Buildings::default();
        let states = |buildings: &Buildings| (buildings.state(house), buildings.state(tower));
        buildings.walk(None);
        assert!(!buildings.changed);
        // An entrance, a stair up, a step from one building's layer onto another's, an exit.
        buildings.walk(Some((house, 1)));
        assert!(std::mem::take(&mut buildings.changed));
        assert_eq!(states(&buildings), (INSIDE, OUTSIDE));
        buildings.walk(Some((house, 1)));
        assert!(!buildings.changed);
        buildings.walk(Some((house, 3)));
        assert_eq!(states(&buildings), (8, OUTSIDE));
        buildings.walk(Some((tower, 2)));
        assert_eq!(states(&buildings), (OUTSIDE, 4));
        buildings.walk(None);
        assert!(std::mem::take(&mut buildings.changed));
        assert_eq!(states(&buildings), (OUTSIDE, OUTSIDE));
        assert!(buildings.states.is_empty());
    }

    /// Heights of sector (0, 0), cell (5, 5) a building of its own and cell (6, 5) a part of it.
    fn heights_with_a_building() -> Heights {
        let cell = |flags, offsets| opensacred_data::world::Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets,
            flags,
            ground: 0,
        };
        let mut cells = vec![cell(0, [0, 0]); SIDE * SIDE];
        cells[5 * SIDE + 5] = cell(BUILDING_CELL, [0, 0]);
        cells[5 * SIDE + 6] = cell(BUILDING_CELL, [-1, 0]);
        let mut heights = Heights::default();
        heights.add(&Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: Vec::new(),
        });
        heights
    }

    #[test]
    fn a_script_gives_the_building_at_a_cell_its_state() {
        let heights = heights_with_a_building();
        let banner = (5, 5);
        let mut buildings = Buildings::default();
        // By a cell of the building's, its anchor's or another's; none by a cell of none.
        buildings.script(&heights, (6, 5), 2);
        assert!(std::mem::take(&mut buildings.changed));
        assert_eq!(buildings.state(banner), INSIDE);
        buildings.script(&heights, (2, 2), 4);
        assert!(!buildings.changed);
        assert_eq!(buildings.states.len(), 1);
        buildings.script(&heights, (5, 5), 1);
        assert!(std::mem::take(&mut buildings.changed));
        assert_eq!(buildings.state(banner), OUTSIDE);
        assert!(buildings.states.is_empty());
        // The hero's steps change it as they change any other's: in and out leaves it 1.
        buildings.script(&heights, (5, 5), 2);
        buildings.walk(Some((banner, 2)));
        assert_eq!(buildings.state(banner), 4);
        buildings.walk(None);
        assert_eq!(buildings.state(banner), OUTSIDE);
    }

    #[test]
    fn a_scripts_state_waits_for_its_cells_sector() {
        let heights = heights_with_a_building();
        let mut buildings = Buildings::default();
        // A cell of sector (1, 0), not loaded, then of (0, 0) as it comes.
        let far = (i32::try_from(SIDE).unwrap() + 3, 4);
        buildings.script(&Heights::default(), (6, 5), 2);
        buildings.script(&heights, far, 2);
        assert!(!buildings.changed);
        assert_eq!(buildings.waiting.len(), 2);
        buildings.arrive(&heights);
        assert!(buildings.changed);
        assert_eq!(buildings.state((5, 5)), INSIDE);
        assert_eq!(buildings.waiting, HashMap::from([(far, 2)]));
    }

    #[test]
    fn a_save_keeps_the_states_and_those_that_wait() {
        let heights = heights_with_a_building();
        let mut buildings = Buildings::default();
        buildings.script(&heights, (6, 5), 2);
        buildings.script(&heights, (300, 4), 2);
        buildings.walk(Some(((9, 9), 2)));
        let saved = buildings.saved();
        assert_eq!(
            saved,
            SavedBuildings {
                states: vec![((5, 5), 2), ((9, 9), 4)],
                waiting: vec![((300, 4), 2)],
            }
        );
        // Loaded with the hero outside: the building it stood in is left as the save had it.
        let mut loaded = Buildings::default();
        loaded.again(saved.clone());
        assert!(loaded.changed);
        assert_eq!(loaded.saved(), saved);
        // Loaded with the hero on another floor, the building's state is that floor's.
        let mut upstairs = inside((9, 9), 3);
        upstairs.again(saved);
        assert_eq!((upstairs.state((5, 5)), upstairs.state((9, 9))), (2, 8));
        // A saved state of 1 is none.
        let mut outside = inside((9, 9), 1);
        outside.again(SavedBuildings {
            states: vec![((5, 5), OUTSIDE)],
            waiting: Vec::new(),
        });
        assert_eq!(outside.saved().states, vec![((9, 9), INSIDE)]);
    }
}
