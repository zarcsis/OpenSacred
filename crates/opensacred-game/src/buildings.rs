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

use bevy::prelude::*;
use opensacred_data::world::{SIDE, Sector};

use crate::frame::Step;
use crate::hero::Body;
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
/// not 0 (`0x62AE90`, docs/re/drawing.md): it shows on the ground always, and on a floor while
/// that is the state's layer.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layered {
    pub building: Option<Anchor>,
    pub layer: i32,
}

impl Layered {
    /// An object at world `(x, z)` on `level`: on a building's floor, that floor; on an inside
    /// layer of no known building's, floor 1 of the building of its cell; on the ground, layer 0.
    pub fn at(heights: &Heights, x: f32, z: f32, level: Level) -> Self {
        let ground = heights.building(x, z).map(|(building, _)| building);
        match (level, heights.floor(level)) {
            (_, Some((building, floor))) => Self {
                building: Some(building),
                layer: i32::from(floor),
            },
            (Some(_), None) => Self {
                building: ground,
                layer: 1,
            },
            (None, None) => Self {
                building: ground,
                layer: 0,
            },
        }
    }

    /// Whether it shows while the buildings are in their states: on the ground, on a building's
    /// cells too, whatever its state; never above the ground where it is in no building.
    pub fn shown(&self, buildings: &Buildings) -> bool {
        match (self.layer, self.building) {
            (0, _) => true,
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

/// The building the hero is in and its floor, from 1, whose state is `1 << floor` ([`INSIDE`]
/// on the ground floor); every other one's is [`OUTSIDE`].
#[derive(Resource, Default)]
pub struct Buildings {
    entered: Option<(Anchor, u8)>,
}

impl Buildings {
    /// A building's state.
    pub fn state(&self, building: Anchor) -> u8 {
        match self.entered {
            Some((entered, floor)) if entered == building => {
                1u8.checked_shl(u32::from(floor)).unwrap_or(INSIDE)
            }
            _ => OUTSIDE,
        }
    }
}

/// Finds the building the hero is in and shows each building's parts and objects of its state,
/// when that changes, as they come and as the objects change layers. The hero is in a building
/// while it walks on an inside layer: in the one whose cell it went onto the layer at, its
/// entrance or, at a start inside, the cell it starts on.
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
    let changed = entered != buildings.entered;
    buildings.entered = entered;
    let buildings = &*buildings;
    let part = |part: &Part, mut shown: Mut<Visibility>| {
        shown.set_if_neq(visibility(part.mask == buildings.state(part.building)));
    };
    let object = |layered: &Layered, mut shown: Mut<Visibility>| {
        shown.set_if_neq(layered.visibility(buildings));
    };
    // All of them when the hero enters or leaves a building, else those that just came or
    // changed layers.
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

/// The buildings' states, by the building the hero is in.
pub struct BuildingsPlugin;

impl Plugin for BuildingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Buildings>()
            .add_systems(Update, enter.in_set(Step::Buildings));
    }
}

#[cfg(test)]
mod tests {
    use opensacred_data::world::{Cell, Layer, LayerCell};

    use super::*;

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
            let buildings = Buildings {
                entered: Some(((1, 1), floor)),
            };
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
        let on = |building, layer| Layered { building, layer };
        // In the building, through its door: its layer 1; on its cell of the ground, its stairs,
        // layer 0; elsewhere, the ground's.
        let novice = at(5.0, Some((0, 0, 1)));
        let stairs = at(5.0, None);
        assert_eq!((novice, stairs), (on(Some(chapel), 1), on(Some(chapel), 0)));
        assert_eq!(at(2.0, None), on(None, 0));
        // The hero outside, inside, upstairs: layer 1 shows inside alone, layer 0 always, on the
        // building's cells too (#148).
        let outside = Buildings::default();
        let inside = Buildings {
            entered: Some((chapel, 1)),
        };
        let upstairs = Buildings {
            entered: Some((chapel, 2)),
        };
        let shown = |object: Layered| {
            [&outside, &inside, &upstairs].map(|buildings| object.shown(buildings))
        };
        assert_eq!(shown(novice), [false, true, false]);
        assert_eq!(shown(stairs), [true; 3]);
        // Off the buildings, the ground's shows always and a layer above it never.
        assert_eq!(shown(on(None, 0)), [true; 3]);
        assert_eq!(shown(on(None, 1)), [false; 3]);
    }
}
