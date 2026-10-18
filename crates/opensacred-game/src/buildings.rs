//! The buildings' states (docs/re/drawing.md, "Buildings: masks and layers"). A cell with flag
//! bit 0 belongs to a building, whose anchor cell its offsets (`+0x1C`, `+0x1D`) point to. The
//! client draws a building's parts (the statics of its cells flagged `0x8`, or of the types of
//! lists 0, 2 and 4, `statics`) only when their mask is the building's state, and the world's
//! objects of its cells from the layer the state gives (the index of its lowest bit). Outside,
//! the state is 1: the shell with its roof. The client sets it to 2 as the hero steps onto an
//! entrance of the building and goes onto its inside layer, and back to 1 as the hero steps
//! onto an exit and comes out onto the ground (`0x5FCCB0`, docs/re/movement.md): the ground
//! floor cut open with its furniture, and the objects of layer 1, while the hero walks inside.
//! The upper floors (states 4 to 16) are not followed (#104).

use bevy::prelude::*;
use opensacred_data::world::{SIDE, Sector};

use crate::frame::Step;
use crate::hero::Body;
use crate::terrain::Heights;

/// The cell flag of a building's cells.
pub const BUILDING_CELL: u8 = 0x1;
/// The state of a building while the hero is outside: its shell's mask.
pub const OUTSIDE: u8 = 1;
/// The state of the building the hero is in.
pub const INSIDE: u8 = 2;

/// A building, by the global cell its parts' cells point to.
pub type Anchor = (u32, u32);

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

/// A start script's object or NPC of a building's layer above 0: shown while the building's
/// state shows `layer`.
#[derive(Component)]
pub struct Furnishing {
    pub building: Anchor,
    pub layer: i32,
}

/// The building the hero is in, whose state is [`INSIDE`]; every other one's is [`OUTSIDE`].
#[derive(Resource, Default)]
pub struct Buildings {
    entered: Option<Anchor>,
}

impl Buildings {
    /// A building's state.
    pub fn state(&self, building: Anchor) -> u8 {
        if self.entered == Some(building) {
            INSIDE
        } else {
            OUTSIDE
        }
    }
}

/// Finds the building the hero is in and shows each building's parts and furnishings of its
/// state, when that changes and as they come. The hero is in a building while it walks on an
/// inside layer: in the one whose cell it went onto the layer at, its entrance or, at a start
/// inside, the cell it starts on.
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
        Query<(&Furnishing, &mut Visibility)>,
        Query<(&Furnishing, &mut Visibility), Added<Furnishing>>,
    )>,
) {
    let entered = bodies.iter().next().and_then(|(hero, body)| {
        body.level()?;
        buildings.entered.or_else(|| {
            heights
                .building(hero.translation.x, hero.translation.z)
                .map(|(building, _)| building)
        })
    });
    let changed = entered != buildings.entered;
    buildings.entered = entered;
    let buildings = &*buildings;
    let show = |visible: bool| {
        if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        }
    };
    let part = |part: &Part, mut visibility: Mut<Visibility>| {
        visibility.set_if_neq(show(part.mask == buildings.state(part.building)));
    };
    let furnishing = |furnishing: &Furnishing, mut visibility: Mut<Visibility>| {
        let state = buildings.state(furnishing.building);
        visibility.set_if_neq(show(furnishing.layer == layer(state)));
    };
    // All of them when the hero enters or leaves a building, else those that just came.
    if changed {
        for (p, visibility) in &mut shown.p0() {
            part(p, visibility);
        }
        for (f, visibility) in &mut shown.p2() {
            furnishing(f, visibility);
        }
    } else {
        for (p, visibility) in &mut shown.p1() {
            part(p, visibility);
        }
        for (f, visibility) in &mut shown.p3() {
            furnishing(f, visibility);
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
    use opensacred_data::world::Cell;

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
        };
        let side = u32::try_from(SIDE).unwrap();
        assert_eq!(
            anchor(&sector, 17 * SIDE + 24),
            Some((50 * side + 19, 39 * side + 15))
        );
        assert_eq!(anchor(&sector, 17 * SIDE + 25), None);
        assert_eq!((layer(1), layer(2), layer(4), layer(16)), (0, 1, 2, 4));
    }
}
