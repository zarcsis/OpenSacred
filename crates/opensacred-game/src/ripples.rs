//! The menus' water, the original's `cConsoleFX` (docs/re/menus.md, "The water"; #1127): a grid
//! of 64 × 64 heights under each menu's picture, into which drops fall by a timer of real time
//! and over which their rings spread, a step at the original's 30 frames a second.
//!
//! A screen's [`Pond`] names its bed, its reflection and its camera; [`Ripples::spawn`] makes its
//! three passes, each mesh drawn by a camera of its own into the screen's texture in turn, so that
//! each lies over the last as the original draws them without a depth test: the bed at the
//! heights, its texture refracted by their slopes; the reflection, moved by them; and the
//! highlights, added where they squeeze the reflection. The grid's points are placed on the CPU
//! where the original's world and camera put them, and the texture, of the shape of the original's
//! frustum, is stretched over the picture as that frustum is over its screen. Its texels hold the
//! original's values as they are (`srgb`), which the interface lays under the picture's tiles.

use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::ecs::system::SystemParam;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

use crate::hud;
use crate::menu::Screen;
use crate::spawning::Rng;

/// The grid's points a side, and the side of its square in the world.
const SIDE: usize = 64;
const SPAN: f32 = 64.0;
/// The disc of points that move: the square of its radius about the point (32, 32).
const DISC: usize = 825;
/// The steps a second, the menus' frames (**seen**), and the most that a frame catches up.
const RATE: f32 = 30.0;
const CATCH_UP: u32 = 4;
/// A step: a speed above `STILL` keeps `DAMPING` of itself, then gains `PULL` of the difference
/// between its neighbours' mean and its height.
const DAMPING: f32 = 0.99;
const STILL: f32 = 0.003;
const PULL: f32 = 0.25;
/// A drop: its row, its column's share of the grid from `COLUMN` and up to `SPREAD` more, what it
/// takes from the speeds of its point, of the four beside it and of the four across, and the
/// seconds to the next, `WAIT` and up to `WAIT_MORE` more (`0x75F7D0`, `0x75EF40`).
const DROP_ROW: usize = 57;
const COLUMN: f32 = 0.3;
const SPREAD: f32 = 0.4;
const STAMP: [f32; 3] = [0.15, 0.075, 0.0325];
const WAIT: f32 = 1.5;
const WAIT_MORE: f32 = 2.5;
/// The refraction that the client's table of Snell's law always gives, the bed's depth under the
/// surface and the point the bed is seen from.
const BEND: f32 = 0.8817;
const SCALE: f32 = -0.7502;
const BED: f32 = 20.0;
const EYE: Vec3 = Vec3::new(0.0, -60.0, 0.0);
/// The sphere that the reflection's rays meet: its centre and the square of its radius.
const SPHERE: Vec3 = Vec3::new(0.0, 20.0, 0.0);
const SPHERE_RADIUS_SQUARED: f32 = 1225.0;
/// A highlight's grey of 255 for a moved cell of area `a`: `GREY` / (k + 1), k four times `a²`
/// cut to a whole number, none from `DARK`.
const GREY: f32 = 196.0;
const DARK: f32 = 16.0;
/// The camera's vertical angle's share of its horizontal one, and its near plane.
const VERTICAL: f32 = 0.75;
const NEAR: f32 = 10.0;
/// The three passes' render layers, after the figures' (`choice`), and their cameras' first
/// order, before the interface's.
const FIRST_LAYER: usize = 2;
const FIRST_ORDER: isize = -6;

/// A screen's camera of the water (`0x75FA70`): its horizontal angle in degrees, the grid's turn
/// about X in radians and its shift along Z, and the eye's distance from the origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub angle: f32,
    pub turn: f32,
    pub distance: f32,
    pub shift: f32,
}

/// The main menu's camera and the choices of the hero's.
pub const MAIN: View = View {
    angle: 45.0,
    turn: 1.43,
    distance: 77.5,
    shift: 14.4,
};
pub const CAVE: View = View {
    angle: 69.7,
    turn: 1.63,
    distance: 47.9,
    shift: 12.4,
};

/// A screen's water: its bed's and its reflection's textures, relative to the asset directory,
/// and its camera.
pub struct Pond<'a> {
    pub bed: Option<&'a String>,
    pub reflection: Option<&'a String>,
    pub view: View,
}

/// The heights and speeds of the grid's points, row by row from the far edge, which the menus'
/// screens share as the original's one effect, the time to the next drop and the time not
/// stepped yet, and the dice.
#[derive(Resource)]
pub struct Surface {
    heights: Vec<f32>,
    speeds: Vec<f32>,
    drop_in: f32,
    owed: f32,
    dice: Rng,
}

impl Default for Surface {
    /// A still surface whose first drop falls at the first update.
    fn default() -> Self {
        Self {
            heights: vec![0.0; SIDE * SIDE],
            speeds: vec![0.0; SIDE * SIDE],
            drop_in: -1.0,
            owed: 0.0,
            dice: Rng::of(0, 0, 0x85),
        }
    }
}

impl Surface {
    /// A number from 0 to 1 at random, as the client's `rand()` / 32767.
    fn roll(&mut self) -> f32 {
        f32::from(u16::try_from(self.dice.below(0x8000)).unwrap_or(0)) / 32_767.0
    }

    /// A drop on the row [`DROP_ROW`] at the column of `share` of the grid: its stamp taken from
    /// the speeds.
    fn drop_at(&mut self, share: f32) {
        // The share times the side, cut to a whole number and kept off the edges.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a whole column of the grid, from 1 to 62"
        )]
        let column = (share * SPAN).floor().clamp(1.0, SPAN - 2.0) as usize;
        for (row, column, take) in stamp(DROP_ROW, column) {
            self.speeds[row * SIDE + column] -= take;
        }
    }

    /// A step of the heights (`0x75F010`): each point of the disc's speed damped while it moves,
    /// then pulled towards its neighbours' mean, the four beside it a sixth each and the four
    /// across a twelfth; then each height takes its speed, less the mean over the grid, which
    /// keeps the surface's level.
    fn step(&mut self) {
        let mut sum = 0.0;
        for row in 0..SIDE {
            for column in 0..SIDE {
                let at = row * SIDE + column;
                if !inside(row, column) {
                    self.heights[at] = 0.0;
                    self.speeds[at] = 0.0;
                    continue;
                }
                let height = |dr: usize, dc: usize| {
                    self.heights[(row + dr) % SIDE * SIDE + (column + dc) % SIDE]
                };
                let (before, after) = (SIDE - 1, 1);
                let beside =
                    height(before, 0) + height(after, 0) + height(0, before) + height(0, after);
                let across = height(before, before)
                    + height(before, after)
                    + height(after, before)
                    + height(after, after);
                let mean = beside / 6.0 + across / 12.0;
                let mut speed = self.speeds[at];
                if speed.abs() > STILL {
                    speed *= DAMPING;
                }
                speed += (mean - self.heights[at]) * PULL;
                self.speeds[at] = speed;
                sum += self.heights[at] + speed;
            }
        }
        let level = sum / grid_points();
        for row in 0..SIDE {
            for column in 0..SIDE {
                if inside(row, column) {
                    let at = row * SIDE + column;
                    self.heights[at] += self.speeds[at] - level;
                }
            }
        }
    }

    /// The normal of the point at `row`, `column`: (gx, 1, gz) normalized, gx and gz the sums of
    /// the three height differences across it, its row's or column's and the two diagonals',
    /// each the lower side's (**hypothesis**: the sign is not read).
    fn normal(&self, row: usize, column: usize) -> Vec3 {
        let height = |r: usize, c: usize| self.heights[r % SIDE * SIDE + c % SIDE];
        let (up, down) = (row + SIDE - 1, row + 1);
        let (left, right) = (column + SIDE - 1, column + 1);
        let gx = (height(row, left) - height(row, right))
            + (height(up, left) - height(up, right))
            + (height(down, left) - height(down, right));
        let gz = (height(up, column) - height(down, column))
            + (height(up, left) - height(down, left))
            + (height(up, right) - height(down, right));
        Vec3::new(gx, 1.0, gz).normalize()
    }
}

/// The points of a drop's stamp at `row`, `column` and what each takes.
fn stamp(row: usize, column: usize) -> [(usize, usize, f32); 9] {
    let [centre, beside, across] = STAMP;
    let (up, down, left, right) = (row - 1, row + 1, column - 1, column + 1);
    [
        (row, column, centre),
        (up, column, beside),
        (down, column, beside),
        (row, left, beside),
        (row, right, beside),
        (up, left, across),
        (up, right, across),
        (down, left, across),
        (down, right, across),
    ]
}

/// Whether the point at `row`, `column` lies within the disc that moves.
fn inside(row: usize, column: usize) -> bool {
    let off = |at: usize| at.abs_diff(SIDE / 2).pow(2);
    off(row) + off(column) < DISC
}

/// The grid's points, as a float.
fn grid_points() -> f32 {
    f32::from(u16::try_from(SIDE * SIDE).unwrap_or(u16::MAX))
}

/// A point's place along a side of the grid, from −32 to 32.
fn along(at: usize) -> f32 {
    f32::from(u16::try_from(at).unwrap_or(0)) * SPAN / (SPAN - 1.0) - SPAN / 2.0
}

/// A screen's water: its camera and its passes' meshes, the bed's, the reflection's and the
/// highlights'.
#[derive(Component)]
struct Water {
    view: View,
    meshes: [Handle<Mesh>; 3],
}

/// What makes a screen's water.
#[derive(SystemParam)]
pub struct Ripples<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
}

impl Ripples<'_> {
    /// The water of `pond` for `screen`, `height` pixels high: its passes and their cameras,
    /// gone with the screen, and the texture they draw into, for the picture to lay under its
    /// tiles.
    pub fn spawn(
        &mut self,
        commands: &mut Commands,
        (images, assets): (&mut Assets<Image>, &AssetServer),
        (screen, pond): (Screen, &Pond),
        height: f32,
    ) -> Handle<Image> {
        let projection = projection(pond.view);
        let size = Vec2::new(height * projection.aspect_ratio, height)
            .round()
            .as_uvec2();
        let target = images.add(Image::new_target_texture(
            size.x,
            size.y,
            TextureFormat::Rgba8Unorm,
            None,
        ));
        let texture = |path: Option<&String>| path.map(|path| hud::image(assets, path.clone()));
        let materials = [
            StandardMaterial {
                base_color_texture: texture(pond.bed),
                unlit: true,
                cull_mode: None,
                ..default()
            },
            StandardMaterial {
                base_color_texture: texture(pond.reflection),
                unlit: true,
                cull_mode: None,
                alpha_mode: AlphaMode::Blend,
                ..default()
            },
            StandardMaterial {
                unlit: true,
                cull_mode: None,
                alpha_mode: AlphaMode::Add,
                ..default()
            },
        ];
        // The reflection's pass only where the screen has one, as the original draws none
        // without its texture.
        let passes = [true, pond.reflection.is_some(), true];
        let meshes = [(); 3].map(|()| self.meshes.add(grid()));
        commands.spawn((
            DespawnOnExit(screen),
            Water {
                view: pond.view,
                meshes: meshes.clone(),
            },
        ));
        for (pass, (material, mesh)) in materials.into_iter().zip(meshes).enumerate() {
            let layer = RenderLayers::layer(FIRST_LAYER + pass);
            let order = FIRST_ORDER + isize::try_from(pass).unwrap_or(0);
            commands.spawn((
                DespawnOnExit(screen),
                Camera3d::default(),
                Camera {
                    order,
                    clear_color: if pass == 0 {
                        ClearColorConfig::Custom(Color::BLACK)
                    } else {
                        ClearColorConfig::None
                    },
                    ..default()
                },
                RenderTarget::from(target.clone()),
                Projection::Perspective(projection.clone()),
                Transform::IDENTITY,
                Tonemapping::None,
                DebandDither::Disabled,
                Msaa::Off,
                layer.clone(),
            ));
            if passes[pass] {
                commands.spawn((
                    DespawnOnExit(screen),
                    Mesh3d(mesh),
                    MeshMaterial3d(self.materials.add(material)),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    layer,
                ));
            }
        }
        target
    }
}

/// The original's projection of `view`: its vertical angle and its frustum's shape, the ratio of
/// its angles' tangents, which the texture takes.
fn projection(view: View) -> PerspectiveProjection {
    let horizontal = view.angle.to_radians();
    let vertical = horizontal * VERTICAL;
    PerspectiveProjection {
        fov: vertical,
        aspect_ratio: (horizontal / 2.0).tan() / (vertical / 2.0).tan(),
        near: NEAR,
        ..default()
    }
}

/// A grid of 64 × 64 points in two triangles a cell, its places, texture coordinates and colours
/// set by [`shape`].
fn grid() -> Mesh {
    let last = u32::try_from(SIDE - 1).unwrap_or(0);
    let side = last + 1;
    let indices: Vec<u32> = (0..last)
        .flat_map(|row| (0..last).map(move |column| (row, column)))
        .flat_map(|(row, column)| {
            let at = row * side + column;
            [at, at + side, at + 1, at + 1, at + side, at + side + 1]
        })
        .collect();
    let points = SIDE * SIDE;
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; points])
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; points])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32; 2]; points])
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32; 4]; points])
    .with_inserted_indices(Indices::U32(indices))
}

/// The camera's space of a point of the grid at (x, y, z): the grid turned about X by the view's
/// turn and shifted along Z, seen from (0, −distance, 0) towards the origin with Z down the
/// screen, as Bevy's camera at the origin sees it, looking down −Z.
fn seen(view: View, point: Vec3) -> [f32; 3] {
    let (sin, cos) = view.turn.sin_cos();
    let y = point.y * cos - point.z * sin;
    let z = point.y * sin + point.z * cos + view.shift;
    [point.x, -z, -(y + view.distance)]
}

/// Steps the surface as the frames' real time owes it and lets a drop fall when its time comes,
/// while a screen shows its water.
fn ripple(time: Res<Time<Real>>, mut surface: ResMut<Surface>) {
    let elapsed = time.delta_secs();
    surface.drop_in -= elapsed;
    if surface.drop_in <= 0.0 {
        let share = COLUMN + SPREAD * surface.roll();
        surface.drop_at(share);
        surface.drop_in = WAIT + WAIT_MORE * surface.roll();
    }
    surface.owed += elapsed;
    let mut steps = 0;
    while surface.owed >= 1.0 / RATE && steps < CATCH_UP {
        surface.step();
        surface.owed -= 1.0 / RATE;
        steps += 1;
    }
    if surface.owed >= 1.0 / RATE {
        surface.owed = 0.0;
    }
}

/// Places each water's points by the surface (`0x75F010`): the bed's at their heights with their
/// texture refracted by their normals, the reflection's moved by them, and the highlights' greys
/// by the areas of the reflection's moved cells.
fn shape(surface: Res<Surface>, waters: Query<&Water>, mut meshes: ResMut<Assets<Mesh>>) {
    for water in &waters {
        let view = water.view;
        let mut bed = Vec::with_capacity(SIDE * SIDE);
        let mut bed_uvs = Vec::with_capacity(SIDE * SIDE);
        let mut moved = Vec::with_capacity(SIDE * SIDE);
        let mut uvs = Vec::with_capacity(SIDE * SIDE);
        for row in 0..SIDE {
            for column in 0..SIDE {
                let (x, z) = (along(column), along(row));
                let height = surface.heights[row * SIDE + column];
                let normal = surface.normal(row, column);
                let point = Vec3::new(x, height, z);
                // The bed, seen through the surface from the refraction's eye.
                let towards = (EYE - point).normalize();
                let bend = SCALE * (towards + BEND * normal);
                bed.push(seen(view, point));
                bed_uvs.push([
                    (x + BED * bend.x) / SPAN + 0.5,
                    (z + BED * bend.z) / SPAN + 0.5,
                ]);
                // The reflection, moved along its ray to the sphere.
                let ray = SCALE * (BEND * normal + Vec3::Y);
                let flat = Vec3::new(x, 0.0, z);
                let off = flat - SPHERE;
                let along_ray = ray.dot(off);
                let reach = along_ray * along_ray - (off.length_squared() - SPHERE_RADIUS_SQUARED);
                let t = if reach >= 0.0 {
                    reach.sqrt() - along_ray
                } else {
                    0.0
                };
                moved.push(Vec2::new(x + ray.x * t, z + ray.z * t));
                uvs.push([
                    f32::from(u16::try_from(column).unwrap_or(0)) / (SPAN - 1.0),
                    f32::from(u16::try_from(row).unwrap_or(0)) / (SPAN - 1.0),
                ]);
            }
        }
        let greys = highlights(&moved);
        let reflected: Vec<[f32; 3]> = moved
            .iter()
            .map(|point| seen(view, Vec3::new(point.x, 0.0, point.y)))
            .collect();
        let [bed_mesh, reflection, lights] = &water.meshes;
        if let Some(mut mesh) = meshes.get_mut(bed_mesh) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, bed);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, bed_uvs);
        }
        if let Some(mut mesh) = meshes.get_mut(reflection) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, reflected.clone());
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        }
        if let Some(mut mesh) = meshes.get_mut(lights) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, reflected);
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_COLOR,
                greys
                    .into_iter()
                    .map(|grey| [grey, grey, grey, 1.0])
                    .collect::<Vec<_>>(),
            );
        }
    }
}

/// The highlights' greys, of 1, of the reflection's points moved to `moved` (x, z): each point's
/// by the area of its moved cell, between it, the point before it in the row before and the one
/// above it; the first row and column take the next's.
fn highlights(moved: &[Vec2]) -> Vec<f32> {
    let mut greys = vec![0.0; SIDE * SIDE];
    for row in 1..SIDE {
        for column in 1..SIDE {
            let at = row * SIDE + column;
            let up = moved[at] - moved[at - SIDE];
            let across = moved[at] - moved[at - SIDE - 1];
            let area = up.perp_dot(across).abs();
            let k = (4.0 * area * area).floor();
            greys[at] = if k < DARK {
                (GREY / (k + 1.0)).floor() / 255.0
            } else {
                0.0
            };
        }
    }
    for column in 0..SIDE {
        greys[column] = greys[SIDE + column.max(1)];
    }
    for row in 0..SIDE {
        greys[row * SIDE] = greys[row * SIDE + 1];
    }
    greys
}

/// The menus' water.
pub struct RipplesPlugin;

impl Plugin for RipplesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Surface>().add_systems(
            Update,
            (ripple, shape).chain().run_if(any_with_component::<Water>),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_still_surface_shows_a_pale_veil() {
        let surface = Surface::default();
        let moved: Vec<Vec2> = (0..SIDE * SIDE)
            .map(|at| Vec2::new(along(at % SIDE), along(at / SIDE)))
            .collect();
        // A still cell's area is (64/63)², k 4, its grey 39 of 255.
        let greys = highlights(&moved);
        assert!(greys.iter().all(|&grey| grey == 39.0 / 255.0));
        assert_eq!(surface.normal(32, 32), Vec3::Y);
    }

    #[test]
    fn a_drop_spreads_a_ring_that_keeps_the_level() {
        let mut surface = Surface::default();
        surface.drop_at(0.5);
        assert_eq!(surface.speeds[DROP_ROW * SIDE + 32], -0.15);
        assert_eq!(surface.speeds[(DROP_ROW - 1) * SIDE + 33], -0.0325);
        for _ in 0..10 {
            surface.step();
        }
        // The ring has spread a few points up the column, the drop's point has risen back, and
        // the mean level stays near 0.
        let column = |row: usize| surface.heights[row * SIDE + 32];
        assert!(column(DROP_ROW - 3).abs() > 0.01);
        assert!(column(DROP_ROW - 8).abs() < 0.01);
        let level: f32 = surface.heights.iter().sum::<f32>() / grid_points();
        assert!(level.abs() < 1e-3, "{level}");
        // The points outside the disc stay still.
        assert_eq!(surface.heights[0], 0.0);
    }

    #[test]
    fn the_main_menus_grid_lies_where_the_original_draws_it() {
        use bevy::camera::CameraProjection;
        // The point z = 20 of the still grid at the row 761 of the 768, its far edge at 499.
        let projection = projection(MAIN);
        let row = |z: f32| {
            let clip = projection.get_clip_from_view()
                * Vec3::from(seen(MAIN, Vec3::new(0.0, 0.0, z))).extend(1.0);
            (1.0 - clip.y / clip.w) / 2.0 * 768.0
        };
        assert!((row(20.0) - 761.0).abs() < 1.0, "{}", row(20.0));
        assert!((row(-32.0) - 499.0).abs() < 1.0, "{}", row(-32.0));
    }
}
