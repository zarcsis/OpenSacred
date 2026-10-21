//! The static objects of a sector (docs/02 §6.1): each object's sprite, assembled from pieces of
//! atlases, drawn with its top-left corner at the object's screen position. The sector's sprites
//! are assembled once when it is built and packed into pages, so that filtering runs across the
//! pieces' edges as within them.
//!
//! The original draws the ground, then the objects back to front. Here a sprite is a quad in the
//! plane of the screen, as flat as the original's, set at the depth of the centre of its cell at
//! height 0, so that the depth buffer orders the sprites and the hero among themselves as the
//! original orders them, by their cells whatever their heights; the ground lies behind all of
//! them (`terrain`), and the miniatures of the liquid cells between it and the liquids' surfaces
//! (`liquids`). Sprites at the same depth, those of one cell and those of the cells of one
//! diagonal, would fight; the shader brings each a little forward in the original's order of
//! drawing (`sprites.wesl`). An upper storey's floor lies in its plane, as the original's does,
//! and what its building's part draws after it stays in front of it ([`Standing`]), as do the
//! models standing on it ([`Floor`]). The sprites of one page share one mesh.
//! Miniatures (flag 0x20) are squares cut from their type's atlas, the miniatures of one atlas
//! one mesh; the animated ones (flag 0x40) show their first frame, still.
//!
//! The original blends a sprite by its alpha, after an alpha test (docs/re/drawing.md,
//! "Blending"). A mesh is drawn twice here (docs/decisions/2026-10-18-sprite-alpha.md): its
//! opaque texels through the depth buffer, then the others blended over what lies behind them,
//! with the blended things of its layer (`blended`), its quads far to near. A light source's
//! miniature (type flag `0x20000`, the lamps) carries a glow, a quad of `PARTICLE_GLOW01` three
//! times its size about it, which `daynight` shows by night (`0x62FF60`, docs/re/daynight.md).
//!
//! A static of a type that casts a shadow has a quad of a cell of the shadow atlas
//! (`SHADOW_TREE00`) at its foot, in list 1 or right before its sprite (docs/re/drawing.md, "The
//! statics' shadows"): blended, through the materials of [`StaticShadows`], which the night fades.
//!
//! The trees and the walls of type 25, the sprites of a type of bit [`FADES`], fade about the
//! hero when they are drawn after it, as the original's do ([`Fade`], `sprites.wesl`).

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::math::DVec3;
use bevy::mesh::{
    Indices, MeshVertexAttribute, MeshVertexBufferLayoutRef, PrimitiveTopology, VertexFormat,
};
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::prelude::{
    AlphaMode, App, Asset, AssetServer, Assets, Commands, Entity, FromWorld, Handle, Image,
    IntoScheduleConfigs, Mesh, Mesh3d, MeshMaterial3d, Plugin, Query, Res, ResMut, Resource,
    Startup, Transform, TypePath, Update, Vec2, Vec3, Vec4, Visibility, With,
};
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, Extent3d, RenderPipelineDescriptor, SpecializedMeshPipelineError,
    TextureDimension, TextureFormat,
};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;
use opensacred_data::world::{SIDE, Sector, Shadow, Sprite, Sprites, StaticObject, World};

use crate::blended::{self, Layer};
use crate::frame::Step;
use crate::terrain::{Rgba, read_png};
use crate::{buildings, iso, liquids};

/// The texture of a batch: an atlas, relative to the asset directory, a page of the sector's
/// sprites, or the shadow atlas, whose materials [`StaticShadows`] holds.
enum BatchTexture {
    Atlas(String),
    Page(usize),
    Shadows,
}

/// The quads of one atlas or page in one layer of the blended things.
struct Batch {
    texture: BatchTexture,
    /// Parts of a building's: the building and their mask (see [`part_of`]).
    part: Option<(buildings::Anchor, u8)>,
    layer: Layer,
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    anchors: Vec<[f32; 4]>,
    sources: Vec<[f32; 4]>,
    screens: Vec<[f32; 2]>,
    fades: Vec<f32>,
    floors: Vec<[f32; 4]>,
    footprints: Vec<[f32; 2]>,
    /// Whether a quad of the batch stands on an upper storey's floor ([`Standing`]).
    on_floor: bool,
    /// Each quad's [`nearness`] and first vertex, until the indices take their order.
    quads: Vec<(f64, u32)>,
    indices: Vec<u32>,
}

/// What places a vertex's sprite in depth (`sprites.wesl`): the centre of its cell at height 0
/// and its draw list's depth, and its object's place in the cell's chain.
pub const ATTRIBUTE_ANCHOR: MeshVertexAttribute = MeshVertexAttribute::new(
    "SpriteAnchor",
    0x5350_5249_5445_4143,
    VertexFormat::Float32x4,
);

/// The texels of the atlas that a vertex's piece may sample: its first column and row, and its
/// last (`sprites.wesl`).
pub const ATTRIBUTE_SOURCE: MeshVertexAttribute = MeshVertexAttribute::new(
    "SpriteSource",
    0x5350_5249_5445_5352,
    VertexFormat::Float32x4,
);

/// Where a vertex lies on the original's screen, in its pixels at zoom 1 (`48 (cx − cy)`,
/// `24 (cx + cy)` less the height): the shader places the vertex on screen from it, so that two
/// sprites whose edges meet at one pixel of the original meet on screen too (`sprites.wesl`).
/// The world position, worked out from the sprite's own cell, rounds apart from its neighbour's
/// by a sixteenth of a unit at the world's coordinates, and a column of pixels between two
/// abutting floors could fall in neither (#113).
pub const ATTRIBUTE_SCREEN: MeshVertexAttribute = MeshVertexAttribute::new(
    "SpriteScreen",
    0x5350_5249_5445_5343,
    VertexFormat::Float32x2,
);

/// Whether a vertex's sprite fades about the hero, 1 or 0: a sprite of a type of bit [`FADES`]
/// (`sprites.wesl`, [`Fade`]).
pub const ATTRIBUTE_FADE: MeshVertexAttribute =
    MeshVertexAttribute::new("SpriteFade", 0x5350_5249_5445_4644, VertexFormat::Float32);

/// For a sprite standing on an upper storey's floor ([`Standing`]), the place of the floor's
/// plane at the vertex's point, brought a little forward: the sprite lies no further back than
/// that over the floor (`sprites.wesl`). A vertex's own anchor for any other sprite.
pub const ATTRIBUTE_FLOOR: MeshVertexAttribute = MeshVertexAttribute::new(
    "SpriteFloor",
    0x5350_5249_5445_464C,
    VertexFormat::Float32x4,
);

/// For a sprite standing on an upper storey's floor, where the ground point under the floor's
/// point at the vertex lies in the cells of the floor's part, from 0 to 1 along each axis inside
/// them (`sprites.wesl`); −1 for any other sprite.
pub const ATTRIBUTE_FOOTPRINT: MeshVertexAttribute = MeshVertexAttribute::new(
    "SpriteFootprint",
    0x5350_5249_5445_4650,
    VertexFormat::Float32x2,
);

/// The shader of [`SpriteMaterial`], embedded in the binary.
const SHADER: &str = "embedded://opensacred_game/sprites.wesl";

/// The material of the sprites of one atlas or page in one of their two passes: unlit, each
/// sprite brought a little forward in the original's order of drawing (`sprites.wesl`).
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(SpriteKey)]
pub struct SpriteMaterial {
    #[texture(0)]
    pub atlas: Handle<Image>,
    /// What the texels are multiplied by: white, but for the statics' shadows, whose alpha the
    /// night takes away (`daynight::shadows`).
    #[uniform(1)]
    pub tint: Vec4,
    /// What the sprites of the types of bit [`FADES`] fade about ([`Fade`]), shared.
    #[storage(2, read_only)]
    pub fade: Handle<ShaderBuffer>,
    pub pass: Pass,
    /// Whether the texels go through the original's alpha test first: a sprite's do, a
    /// miniature's do not (docs/re/drawing.md, "Blending").
    pub alpha_test: bool,
    /// Whether some of the sprites stand on an upper storey's floor ([`Standing`]): their
    /// fragments then take their depth from the shader, which keeps them in front of the floor.
    pub on_floor: bool,
}

/// The texels of its sprites that a [`SpriteMaterial`] draws.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pass {
    /// The opaque ones, through the depth buffer.
    Body,
    /// The others, blended by their alpha over what lies behind them, in their layer of the
    /// blended things.
    Fringe(Layer),
}

/// What the pipeline of a [`SpriteMaterial`] depends on.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpriteKey {
    fringe: bool,
    alpha_test: bool,
    on_floor: bool,
}

impl From<&SpriteMaterial> for SpriteKey {
    fn from(material: &SpriteMaterial) -> Self {
        Self {
            fringe: material.pass != Pass::Body,
            alpha_test: material.alpha_test,
            on_floor: material.on_floor,
        }
    }
}

impl Material for SpriteMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        match self.pass {
            Pass::Body => AlphaMode::Opaque,
            Pass::Fringe(_) => AlphaMode::Blend,
        }
    }

    fn depth_bias(&self) -> f32 {
        match self.pass {
            Pass::Body => 0.0,
            Pass::Fringe(layer) => layer.bias(),
        }
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            ATTRIBUTE_ANCHOR.at_shader_location(2),
            ATTRIBUTE_SOURCE.at_shader_location(3),
            ATTRIBUTE_SCREEN.at_shader_location(4),
            ATTRIBUTE_FADE.at_shader_location(5),
            ATTRIBUTE_FLOOR.at_shader_location(6),
            ATTRIBUTE_FOOTPRINT.at_shader_location(7),
        ])?];
        descriptor.primitive.cull_mode = None;
        let SpriteKey {
            fringe,
            alpha_test,
            on_floor,
        } = key.bind_group_data;
        if let Some(fragment) = descriptor.fragment.as_mut() {
            // In both passes, which take the texels that the test leaves.
            if alpha_test {
                fragment.shader_defs.push("ALPHA_TEST".into());
            }
            if fringe {
                fragment.shader_defs.push("FRINGE".into());
            }
            // Writing the depth turns the early depth test off: only for the sprites that need it.
            let entry = if on_floor {
                "fragment_on_floor"
            } else {
                "fragment"
            };
            fragment.entry_point = Some(entry.into());
        }
        if fringe {
            // Only nearer than the depth there: the sprite's own opaque texels, which the first
            // pass drew at the same depth, are passed over before the shader runs.
            if let Some(depth) = descriptor.depth_stencil.as_mut() {
                depth.depth_compare = Some(CompareFunction::Greater);
            }
        }
        Ok(())
    }
}

/// The sprites' material and its shader.
pub struct StaticsPlugin;

impl Plugin for StaticsPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "sprites.wesl");
        app.add_plugins(MaterialPlugin::<SpriteMaterial>::default())
            .init_resource::<Fade>()
            .add_systems(Startup, make_shadows)
            .add_systems(Update, fade.in_set(Step::Models));
    }
}

/// The sprite meshes of a sector, built on the CPU, and the pages of its sprites.
#[derive(Default)]
pub struct Statics {
    batches: Vec<Batch>,
    pages: Vec<Image>,
    /// The floors that the quads of the buildings' parts stand on, for the models.
    floors: Vec<Floor>,
    /// The glows over the light sources' miniatures, for [`spawn_glows`].
    pub glows: Vec<Glows>,
    pub objects: usize,
    pub drawn: usize,
}

/// The glows over the light sources' miniatures of a building's part, or of none.
pub struct Glows {
    part: Option<(buildings::Anchor, u8)>,
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

/// The glows' material, one for all (`daynight::glow` keeps its colour by the night).
#[derive(Resource)]
pub struct Glow(pub Handle<blended::Sorted>);

/// The type flag of the light sources (`items.pak +0x00`), whose miniatures glow.
const LIGHT: u32 = 0x2_0000;
/// How far forward a glow comes, in world units, past the sprites of its cell (`sprites.wesl`
/// brings them forward a few units at most): over its own lamp, under the next diagonal's.
const GLOW_FORWARD: f32 = 8.0;

/// `dir/relative` with `..` resolved, relative to `root`, with `/` separators.
fn asset_path(root: &Path, dir: &Path, relative: &str) -> Result<String> {
    let mut path = PathBuf::new();
    for component in dir.join(relative).components() {
        match component {
            Component::ParentDir => {
                path.pop();
            }
            other => path.push(other),
        }
    }
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("{} lies outside {}", path.display(), root.display()))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

/// Width and height of a PNG, from its header.
fn png_size(path: &Path) -> Result<(f32, f32)> {
    let file = fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .with_context(|| format!("reading {}", path.display()))?;
    let info = reader.info();
    Ok((info.width as f32, info.height as f32))
}

/// The objects and their children, depth first.
fn flatten<'a>(objects: &'a [StaticObject], out: &mut Vec<&'a StaticObject>) {
    for object in objects {
        out.push(object);
        flatten(&object.children, out);
    }
}

/// Reads the sprite table, `world/sprites.bin`.
pub fn read_sprites(assets: &Path) -> Result<Sprites> {
    let table = assets
        .join("world")
        .join(opensacred_data::world::SPRITES_FILE);
    let data = fs::read(&table).with_context(|| format!("reading {}", table.display()))?;
    Sprites::decode(&data).with_context(|| format!("decoding {}", table.display()))
}

/// Flags of objects the original does not draw with the world's art (`0x0062AE90`, openheilig).
const HIDDEN: u32 = 0x290;
/// The object flag of a building's parts of list 3 that the building's state chooses by their
/// mask (docs/re/drawing.md).
const BUILDING_PART: u32 = 0x8;

/// The building and the mask of `object`, if it is a part of a building: in a building's cell,
/// an object flagged [`BUILDING_PART`] or of a type of lists 0, 2 and 4 ([`UNDERLAY`],
/// [`OVERLAY`]), whatever its flags. The client draws it only while its building's state is its
/// mask (`0x62AE90`): the shells have mask 1, the rooms cut open and their furniture the masks
/// above, and the miniatures under a building mask 0, which no state shows.
fn part_of(object: &StaticObject, sector: &Sector) -> Option<(buildings::Anchor, u8)> {
    if object.flags & BUILDING_PART == 0 && object.type_flags & (UNDERLAY | OVERLAY) == 0 {
        return None;
    }
    buildings::anchor(sector, usize::from(object.cell)).map(|building| (building, object.mask))
}
/// A miniature: a square of its type's atlas instead of a sprite.
const MINIATURE: u32 = 0x20;
/// An animated miniature: a grid of frames, rotated.
const ANIMATED: u32 = 0x40;
/// The side of a miniature atlas, fixed in the original (openheilig).
const MINIATURE_ATLAS: f32 = 256.0;
/// The bits of an object type's flags (its `items.pak` record `+0x00` as a u32) that put its
/// objects in the original's draw lists other than 3 (LGP `sub_80E0E96`, openheilig): `0x4`
/// under the ordinary sprites, in list 0 for a miniature or an object of mask 1, in list 2 for
/// the rest; `0x800000` in list 4, over them.
const UNDERLAY: u32 = 0x4;
const OVERLAY: u32 = 0x80_0000;
/// How far back along the line of sight the objects of lists 0, 1 (the statics' shadows) and 2
/// go: under the sprites of list 3 and the hero, above the ground, their order within a list
/// unchanged, each list a thousand units (30 diagonals) from the next.
pub const LIST_0_BACK: f32 = 4_000.0;
pub const LIST_1_BACK: f32 = 3_000.0;
pub const LIST_2_BACK: f32 = 2_000.0;
const _: () = assert!(LIST_2_BACK < LIST_1_BACK && LIST_1_BACK < LIST_0_BACK);
/// How far back the miniatures of list 0 in a liquid cell go: the original draws them before
/// the liquids' surfaces (`0x62E410`, docs/re/drawing.md), so they lie between the ground and the
/// surfaces (`liquids::BACK`), their order among themselves that of list 0. A quad stands in the
/// plane of the screen while the bed and the surface slope away under it, a unit and a third
/// nearer per pixel down the screen: the margins hold for the tallest of these miniatures, from
/// 68 pixels above their cell's centre to 139 below, over a bed up to 61 pixels off their cell's
/// height, their cells from 67.5 pixels below height 0, where the anchors lie, to 25 above: a
/// pixel of its cell's height takes a quad 0.46 units further back (counted on the sector files
/// 2026-10-18).
pub const UNDER_LIQUIDS_BACK: f32 = 4_600.0;
/// How far forward the objects of list 4 come: over the sprites of list 3 around them. Whether
/// the original draws them over the characters too is not known.
const LIST_4_FORWARD: f32 = 1_000.0;

/// The original's order of drawing as the sprites' shader has it (`order.wesl`): how far forward
/// a place comes per world unit its cell lies right, and per place in its cell's chain.
const ACROSS: f64 = 0.002;
const CHAIN: f64 = 0.0045;

/// How near the camera the place `anchor` lies in the order, as the sprites' shader puts it up to
/// a constant (`order.wesl`): a batch's quads go far to near, so that each one's blended texels
/// lie over those behind them.
fn nearness(anchor: [f32; 4]) -> f64 {
    let [x, y, z, chain] = anchor.map(f64::from);
    let at = DVec3::new(x, y, z);
    at.dot(-iso::forward().as_dvec3()) + ACROSS * at.dot(iso::right().as_dvec3()) + CHAIN * chain
}

/// The width of a page of a sector's sprites, and the most it grows to before another begins.
const PAGE: usize = 2048;
/// Transparent texels between two sprites of a page.
const GUTTER: usize = 1;

/// Where a quad's texels come from: a miniature's atlas, a page of the sector's sprites, or the
/// shadow atlas.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Texture {
    Atlas(u32),
    Page(usize),
    Shadows,
}

/// What the quads of a batch share: their texture, their building's part and their layer.
type BatchKey = (Texture, Option<(buildings::Anchor, u8)>, Layer);

/// A quad to draw: its texture, the corners on screen (top-left, top-right, bottom-right,
/// bottom-left) relative to the object's position, and the rectangle of the texture in pixels.
struct Quad {
    texture: Texture,
    corners: [Vec2; 4],
    source: [f32; 4],
    /// A miniature's size across on screen, by which its glow reaches past it.
    size: Option<f32>,
    /// Whether it fades about the hero: a sprite of a type of bit [`FADES`].
    fades: bool,
}

/// A sprite assembled from its pieces, one to one, over its rectangle: so that filtering blends
/// the texels on either side of a piece's edge as it does inside a piece. Filtering each piece in
/// its own atlas could not reach the next piece's texels, and the step between two pieces showed
/// as a line once magnified. The rectangle is the sprite's size from its pieces' smallest corner,
/// past which the pieces' blocks hold transparent texels only (docs/02 §3.3): where the art is
/// cut at the rectangle's edge to go on in another sprite, its edge's texels then reach the edge,
/// as the original draws them, rather than fading out into those texels half a texel short of
/// it, where what lay behind both sprites showed through as a line (#35).
struct Assembled {
    /// The rectangle in the sprite's pixels: `x0, y0, x1, y1`.
    rect: [i32; 4],
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

/// The sprite's pieces copied from their atlases into one image; `None` without pieces.
fn assemble(sprite: &Sprite, atlases: &mut Atlases<'_>) -> Result<Option<Assembled>> {
    let mut covered: Option<[i32; 4]> = None;
    for piece in &sprite.pieces {
        let [x0, y0, x1, y1] = piece.rect.map(i32::from);
        covered = Some(match covered {
            None => [x0, y0, x1, y1],
            Some([a, b, c, d]) => [a.min(x0), b.min(y0), c.max(x1), d.max(y1)],
        });
    }
    let Some([x0, y0, x1, y1]) = covered else {
        return Ok(None);
    };
    let rect = [
        x0,
        y0,
        x1.min(x0 + i32::from(sprite.width)),
        y1.min(y0 + i32::from(sprite.height)),
    ];
    let width = usize::try_from(rect[2] - rect[0])?;
    let height = usize::try_from(rect[3] - rect[1])?;
    let mut pixels = vec![0u8; 4 * width * height];
    for piece in &sprite.pieces {
        let atlas = atlases.get(piece.atlas)?;
        let [u0, v0, u1, v1] = piece.source.map(usize::from);
        let x = usize::try_from(i32::from(piece.rect[0]) - rect[0])?;
        let y = usize::try_from(i32::from(piece.rect[1]) - rect[1])?;
        // The part of the piece within the rectangle and the atlas.
        let u1 = u1.min(atlas.width).min((u0 + width).saturating_sub(x));
        let v1 = v1.min(atlas.height).min((v0 + height).saturating_sub(y));
        if u1 <= u0 {
            continue;
        }
        for v in v0..v1 {
            let from = 4 * (v * atlas.width + u0);
            let to = 4 * (v * atlas.width + u1);
            let at = 4 * ((y + v - v0) * width + x);
            pixels[at..at + to - from].copy_from_slice(&atlas.pixels[from..to]);
        }
    }
    Ok(Some(Assembled {
        rect,
        width,
        height,
        pixels,
    }))
}

/// The atlases of the sprites' pieces, decoded on first use.
struct Atlases<'a> {
    dir: &'a Path,
    world: &'a World,
    loaded: HashMap<u32, Rgba>,
}

impl Atlases<'_> {
    fn get(&mut self, atlas: u32) -> Result<&Rgba> {
        if !self.loaded.contains_key(&atlas) {
            let name = self
                .world
                .atlases
                .get(usize::try_from(atlas)?)
                .with_context(|| format!("no atlas {atlas}"))?;
            let image = read_png(&self.dir.join(name))?;
            self.loaded.insert(atlas, image);
        }
        self.loaded
            .get(&atlas)
            .with_context(|| format!("atlas {atlas}"))
    }
}

/// Where each sprite went on the pages: its page and the top-left corner of its image there.
type Placed = HashMap<u32, (usize, usize, usize)>;

/// A page of a sector's sprites: their images side by side in rows, a gutter apart.
struct Page {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

/// Packs the sprites into pages, the tallest first along rows; returns the pages and where each
/// sprite went: its page and the top-left corner of its image there.
fn pack(sprites: &BTreeMap<u32, Assembled>) -> (Vec<Page>, Placed) {
    let mut order: Vec<(&u32, &Assembled)> = sprites.iter().collect();
    order.sort_by_key(|(id, s)| (std::cmp::Reverse(s.height), **id));
    let mut placed = HashMap::new();
    // The pages' heights so far, and the row being filled: its top, its height, its next x.
    let mut heights: Vec<usize> = Vec::new();
    let (mut top, mut row, mut x) = (0, 0, 0);
    for (&id, sprite) in order {
        let (w, h) = (sprite.width + GUTTER, sprite.height + GUTTER);
        if heights.is_empty() || x + w > PAGE {
            top += row;
            x = 0;
            row = 0;
        }
        if heights.is_empty() || top + h > PAGE {
            heights.push(0);
            top = 0;
            x = 0;
            row = 0;
        }
        let page = heights.len() - 1;
        placed.insert(id, (page, x, top));
        x += w;
        row = row.max(h);
        heights[page] = heights[page].max(top + row);
    }
    let mut pages: Vec<Page> = heights
        .into_iter()
        .map(|height| Page {
            width: PAGE,
            height,
            pixels: vec![0; 4 * PAGE * height],
        })
        .collect();
    for (id, &(page, x, y)) in &placed {
        let sprite = &sprites[id];
        let page = &mut pages[page];
        for row in 0..sprite.height {
            let from = 4 * row * sprite.width;
            let at = 4 * ((y + row) * page.width + x);
            page.pixels[at..at + 4 * sprite.width]
                .copy_from_slice(&sprite.pixels[from..from + 4 * sprite.width]);
        }
    }
    (pages, placed)
}

/// The sprite of an object, if it has one with pieces and is drawn as a sprite.
fn sprite_of<'a>(object: &StaticObject, sprites: &'a Sprites) -> Option<(u32, &'a Sprite)> {
    if object.flags & (HIDDEN | MINIATURE) != 0 || object.sprite == 0 {
        return None;
    }
    let sprite = sprites
        .sprites
        .get(usize::try_from(object.sprite).ok()?)?
        .as_ref()?;
    (!sprite.pieces.is_empty()).then_some((object.sprite, sprite))
}

/// The quad of an object: its assembled sprite on its page, or its miniature's square; none
/// when the object has neither or is not drawn.
fn quad(
    object: &StaticObject,
    assembled: &BTreeMap<u32, Assembled>,
    placed: &Placed,
) -> Option<Quad> {
    if object.flags & HIDDEN != 0 {
        return None;
    }
    if object.flags & MINIATURE != 0 {
        let mini = object.miniature?;
        let [a, b, c, _, _] = mini.params.map(f32::from);
        let (size, source, angle) = if object.flags & ANIMATED != 0 {
            // Columns and rows of frames, a rotation in 1/256 turns, timing and frames: the first
            // frame is drawn, still.
            if a == 0.0 || b == 0.0 {
                return None;
            }
            let size = Vec2::new(MINIATURE_ATLAS / a, MINIATURE_ATLAS / b);
            (
                size,
                [0.0, 0.0, size.x, size.y],
                c * std::f32::consts::TAU / 256.0,
            )
        } else {
            (Vec2::splat(c), [a, b, a + c, b + c], 0.0)
        };
        // Rotated about the position, screen y down.
        let (sin, cos) = angle.sin_cos();
        let turn = |p: Vec2| Vec2::new(p.x * cos + p.y * sin, p.y * cos - p.x * sin);
        let corners = [
            Vec2::ZERO,
            Vec2::new(size.x, 0.0),
            size,
            Vec2::new(0.0, size.y),
        ]
        .map(turn);
        return Some(Quad {
            texture: Texture::Atlas(mini.atlas),
            corners,
            source,
            size: Some(size.x),
            fades: object.type_flags & FADES != 0,
        });
    }
    let sprite = assembled.get(&object.sprite)?;
    let &(page, x, y) = placed.get(&object.sprite)?;
    let [x0, y0, x1, y1] = sprite.rect.map(|v| v as f32);
    let (u0, v0) = (x as f32, y as f32);
    Some(Quad {
        texture: Texture::Page(page),
        corners: [
            Vec2::new(x0, y0),
            Vec2::new(x1, y0),
            Vec2::new(x1, y1),
            Vec2::new(x0, y1),
        ],
        source: [u0, v0, u0 + sprite.width as f32, v0 + sprite.height as f32],
        size: None,
        fades: object.type_flags & FADES != 0,
    })
}

/// The bit of a type's flags whose statics cast a shadow, and the static's flag that takes it
/// away (`0x62AE90`).
const CASTS_SHADOW: u32 = 0x1_0000;
const NO_SHADOW: u32 = 0x800;
/// The shadow atlas's cells a side, and their side in texels (`SHADOW_TREE00`).
const SHADOW_CELLS: u16 = 16;
const SHADOW_CELL: f32 = 32.0;

/// Where the original draws a static's shadow (`0x62AE90`, `0x62FF60`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Entry {
    /// An entry of kind 8 in list 1, for a static of mask 1, whether it is drawn or not.
    List1,
    /// With its sprite, right before it, in the sprite's list (an entry of kind `0x11`), for a
    /// sprite of mask 2 or more: the buildings' furniture.
    WithSprite,
}

/// The shadow quad of `object` and where it goes, for an object of a type that casts one, without
/// flag `0x800`, of mask 1, or of mask 2 or more drawn as a sprite (`quad`); `DETAILLEVEL` is 1
/// or more, as the GOG settings have it. A shadow of size 0 draws nothing.
fn shadow_of(object: &StaticObject, quad: Option<&Quad>) -> Option<(Quad, Entry)> {
    let shadow = object.shadow?;
    if object.type_flags & CASTS_SHADOW == 0
        || object.flags & (NO_SHADOW | HIDDEN) != 0
        || shadow.size == 0
    {
        return None;
    }
    let entry = match object.mask {
        1 => Entry::List1,
        2.. if quad.is_some_and(|q| matches!(q.texture, Texture::Page(_))) => Entry::WithSprite,
        _ => return None,
    };
    Some((shadow_quad(&shadow), entry))
}

/// The quad of `shadow` relative to its static's position (`0x631EB0`): `2s` wide and `s` tall
/// for a size `s`, its bottom edge on the foot's line and centred on the foot, its top edge too
/// for a rectangle, shifted right by `s` for a shadow that leans; the whole of its cell of the
/// atlas over it.
fn shadow_quad(shadow: &Shadow) -> Quad {
    let s = f32::from(shadow.size);
    let foot = Vec2::new(shadow.foot[0] as f32, shadow.foot[1] as f32);
    let top = if shadow.leaning { foot.x } else { foot.x - s };
    let (row, column) = (shadow.cell / SHADOW_CELLS, shadow.cell % SHADOW_CELLS);
    let (u, v) = (
        f32::from(column) * SHADOW_CELL,
        f32::from(row) * SHADOW_CELL,
    );
    Quad {
        texture: Texture::Shadows,
        corners: [
            Vec2::new(top, foot.y - s),
            Vec2::new(top + 2.0 * s, foot.y - s),
            Vec2::new(foot.x + s, foot.y),
            Vec2::new(foot.x - s, foot.y),
        ],
        source: [u, v, u + SHADOW_CELL, v + SHADOW_CELL],
        size: None,
        fades: false,
    }
}

/// How far behind the ground point under it an upper storey's floor lies, along the line of
/// sight: a cell's depth from its centre to its front corner (33 units), the half unit that a
/// model spreads about its place (`models.wesl`) and the room of the quads that stand on the
/// floor ([`ON_FLOOR`], [`FLOOR_ROOM`]), so that what stands anywhere on a cell of the floor, at
/// its cell's place, lies in front of the floor under it and of the quads standing on it.
const FLOOR_BEHIND: f32 = 37.0;

/// How far in front of an upper storey's floor the quads standing on it lie at least
/// ([`Standing`]): past the floor's own place in its cell's chain, 32 places of [`CHAIN`] at most,
/// 0.14 units.
const ON_FLOOR: f32 = 0.5;
/// The room over which the quads standing on a floor come forward from [`ON_FLOOR`], in their
/// order of drawing.
const FLOOR_ROOM: f32 = 2.5;
const _: () = assert!(FLOOR_BEHIND > 33.0 + 0.5 + ON_FLOOR + FLOOR_ROOM);

/// How far in front of a floor's plane what stands on it lies, at rank `rank` of the `count`
/// quads standing on it in the order of drawing: a quad at the middle of its step, `rank` whole,
/// and a model drawn after `rank` of them at the bound between two steps ([`Floor`]).
fn raise(rank: f32, count: usize) -> f32 {
    ON_FLOOR + FLOOR_ROOM * rank / count.max(1) as f32
}

/// The floor that a quad stands on: the first upper storey's floor in the order of drawing of
/// the quad's building's part, which the original draws through the depth buffer before the
/// quad, and then the quad over it whole, without the depth test (`0x62FF60`, docs/re/drawing.md,
/// "Depth"). Over the floor's part the quad lies at least `raise` in front of the floor's plane,
/// in the shader (`sprites.wesl`).
#[derive(Clone, Copy)]
struct Standing {
    /// The height of the floor's plane in pixels, and its list's depth.
    height: f32,
    back: f32,
    /// How far in front of the floor's plane: from [`ON_FLOOR`], a little further for each quad
    /// that the original draws after another on the floor.
    raise: f32,
    /// The cells of the floor's part, over which the floor lies: the first column and row, and
    /// those past the last.
    cells: [f32; 4],
}

/// The floor that the quads of a building's part stand on ([`Standing`]), for the models that
/// stand on it (`models::place`): the original draws a model on the floor after the floor, which
/// tests the depth in its plane, and the model above the plane shows whole (docs/re/drawing.md,
/// "Depth"). A model lies no further back than the floor's plane over its whole extent, among the
/// quads standing on the floor in its order of drawing (`models.wesl`). An entity of its sector.
#[derive(bevy::prelude::Component, Clone, Debug)]
pub struct Floor {
    /// The building and the part's mask.
    pub part: (buildings::Anchor, u8),
    /// The height of the floor's plane in pixels, and its list's depth.
    height: f32,
    back: f32,
    /// The [`nearness`] of each quad standing on the floor, far to near.
    places: Vec<f64>,
}

impl Floor {
    /// For a model whose place in the order is `place` (`models::place_of`), as the models'
    /// shader takes it (`models.wesl`): the height of the floor's plane in world units, how far
    /// behind the ground point under a point of the plane the model lies at most, in its order
    /// among the quads standing on the floor, and 1.
    pub fn under(&self, place: Vec4) -> Vec4 {
        let near = nearness(place.to_array());
        let before = self.places.partition_point(|&p| p < near);
        let raise = raise(before as f32, self.places.len());
        Vec4::new(
            iso::height(self.height),
            self.back + FLOOR_BEHIND - raise,
            0.0,
            1.0,
        )
    }
}

/// A quad's place: the centre of its cell at height 0, at its list's depth, with its place in
/// the cell, and that centre on the original's screen and in cells; for an upper storey's floor,
/// the height of its plane in pixels; for a quad standing on one, that floor.
struct Spot {
    anchor: Vec3,
    back: f32,
    chain: f32,
    screen: Vec2,
    cell: Vec2,
    floor: Option<f32>,
    standing: Option<Standing>,
}

/// The ground point, in cells, under the point at `screen` of a plane `height` pixels high, both
/// on the original's screen at zoom 1: `48 (cx − cy)`, `24 (cx + cy)`.
fn under(screen: Vec2, height: f32) -> Vec2 {
    let (across, down) = (screen.x / 48.0, (screen.y + height) / 24.0);
    Vec2::new((down + across) / 2.0, (down - across) / 2.0)
}

impl Spot {
    /// The spot of cell centre `(cx, cy)`, `back` units back along the line of sight, at place
    /// `chain` of the cell.
    fn new(cx: f32, cy: f32, back: f32, chain: f32) -> Self {
        Self {
            anchor: iso::ground(cx, cy) + iso::forward() * back,
            back,
            chain,
            screen: Vec2::new(48.0 * (cx - cy), 24.0 * (cx + cy)),
            cell: Vec2::new(cx, cy),
            floor: None,
            standing: None,
        }
    }

    /// The spot of an upper storey's floor whose plane lies `floor` units of the models' height
    /// high, if any.
    fn on_floor(self, floor: Option<u16>) -> Self {
        Self {
            floor: floor.map(|units| f32::from(units) * iso::PIXELS_PER_MODEL_HEIGHT),
            ..self
        }
    }

    /// The anchor as the shader takes it, its place in the cell last.
    fn placed(&self) -> [f32; 4] {
        [self.anchor.x, self.anchor.y, self.anchor.z, self.chain]
    }

    /// The anchor of the quad's corner at `screen`, a point of the original's screen at zoom 1:
    /// the quad's own, but for an upper storey's floor, which the original draws through the
    /// depth buffer in a horizontal plane at its height (`0x62FF60`, `0x6288A0`). Each corner of
    /// a floor takes the place of the ground point under the floor's point there, `FLOOR_BEHIND`
    /// further back: what stands on the floor, at its cell's place, lies in front of the floor
    /// wherever it stands, and the floor's points further up the screen, further back on the
    /// floor, lie behind what stands before them.
    fn anchor_at(&self, screen: Vec2) -> [f32; 4] {
        let Some(height) = self.floor else {
            return self.placed();
        };
        let cell = under(screen, height);
        let ground = iso::ground(cell.x, cell.y) + iso::forward() * (self.back + FLOOR_BEHIND);
        [ground.x, ground.y, ground.z, self.chain]
    }

    /// For a quad standing on a floor, at its corner at `screen`: the place of the floor's
    /// plane there, `raise` forward, at the first place of the cell's chain, whose depth the
    /// shader keeps the quad in front of over the floor's part; and where the ground point under
    /// that point lies in the part's cells, from 0 to 1 along each axis inside them. For any other
    /// quad its own place, and outside.
    fn floor_at(&self, screen: Vec2) -> ([f32; 4], [f32; 2]) {
        let Some(on) = self.standing else {
            return (self.placed(), [-1.0; 2]);
        };
        let cell = under(screen, on.height);
        let plane =
            iso::ground(cell.x, cell.y) + iso::forward() * (on.back + FLOOR_BEHIND - on.raise);
        let [x0, y0, x1, y1] = on.cells;
        (
            [plane.x, plane.y, plane.z, 0.0],
            [(cell.x - x0) / (x1 - x0), (cell.y - y0) / (y1 - y0)],
        )
    }

    /// The point of the world in the plane of the screen through the anchor that shows at
    /// `screen`, a point of the original's screen at zoom 1.
    fn at(&self, screen: Vec2) -> [f32; 3] {
        let p = self.anchor
            + iso::right() * ((screen.x - self.screen.x) / iso::PIXELS_PER_UNIT)
            + iso::up() * ((self.screen.y - screen.y) / iso::PIXELS_PER_UNIT);
        p.to_array()
    }
}

/// A quad of an object on its way to its batch `key` (of `texture`, `size` texels across and
/// down), at `spot`, once the floors that the quads of its building's part stand on are known
/// ([`stand_on_floors`]).
struct Pending {
    key: BatchKey,
    texture: BatchTexture,
    size: (f32, f32),
    quad: Quad,
    /// The object's position.
    position: Vec2,
    spot: Spot,
}

/// Stands the quads of each building's part on its first upper storey's floor in the order of
/// drawing, if it has one: those that the original draws after it ([`Standing`]), the floor's
/// own pieces aside. They lie in front of the floor's plane over the cells of the part, each a
/// little further forward than the one before, so that they keep their order there. Returns the
/// floors, for the models standing on them.
fn stand_on_floors(pending: &mut [Pending]) -> Vec<Floor> {
    /// A part's first floor, at its place, height and list's depth, and the cells of its quads.
    struct Part {
        first: Option<(f64, f32, f32)>,
        cells: [f32; 4],
    }
    let mut parts: HashMap<(buildings::Anchor, u8), Part> = HashMap::new();
    for quad in pending.iter() {
        let Some(part) = quad.key.1 else {
            continue;
        };
        let Part { first, cells } = parts.entry(part).or_insert(Part {
            first: None,
            cells: [f32::MAX, f32::MAX, f32::MIN, f32::MIN],
        });
        let c = quad.spot.cell;
        *cells = [
            cells[0].min(c.x - 0.5),
            cells[1].min(c.y - 0.5),
            cells[2].max(c.x + 0.5),
            cells[3].max(c.y + 0.5),
        ];
        if let Some(height) = quad.spot.floor {
            let place = nearness(quad.spot.placed());
            if first.is_none_or(|(before, _, _)| place < before) {
                *first = Some((place, height, quad.spot.back));
            }
        }
    }
    let mut floors = Vec::new();
    for (part, Part { first, cells }) in parts {
        let Some((floor, height, back)) = first else {
            continue;
        };
        let mut on: Vec<(f64, usize)> = pending
            .iter()
            .enumerate()
            .filter(|(_, q)| q.key.1 == Some(part) && q.spot.floor.is_none())
            .map(|(i, q)| (nearness(q.spot.placed()), i))
            .filter(|&(place, _)| place > floor)
            .collect();
        on.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (k, &(_, i)) in on.iter().enumerate() {
            pending[i].spot.standing = Some(Standing {
                height,
                back,
                raise: raise(k as f32 + 0.5, on.len()),
                cells,
            });
        }
        floors.push(Floor {
            part,
            height,
            back,
            places: on.into_iter().map(|(place, _)| place).collect(),
        });
    }
    floors
}

/// Adds the quad of `pending` to its batch.
fn add_quad(batches: &mut BTreeMap<BatchKey, Batch>, pending: Pending) -> Result<()> {
    let Pending {
        key,
        texture,
        size,
        quad,
        position,
        spot,
    } = pending;
    let (_, part, layer) = key;
    let batch = batches.entry(key).or_insert(Batch {
        texture,
        part,
        layer,
        positions: Vec::new(),
        uvs: Vec::new(),
        anchors: Vec::new(),
        sources: Vec::new(),
        screens: Vec::new(),
        fades: Vec::new(),
        floors: Vec::new(),
        footprints: Vec::new(),
        on_floor: false,
        quads: Vec::new(),
        indices: Vec::new(),
    });
    // The texels map onto the quad one to one; the shader samples only the texels whose centres
    // lie within the quad's source, where filtering would blend in the texels around them.
    let [u0, v0, u1, v1] = quad.source;
    let first = u32::try_from(batch.positions.len())?;
    let screens = quad.corners.map(|corner| position + corner);
    batch
        .positions
        .extend(screens.map(|screen| spot.at(screen)));
    // Whole pixels for a sprite, exact in an `f32` across the world's 307,200 of them.
    batch
        .screens
        .extend(screens.map(|screen| screen.to_array()));
    batch.fades.extend([f32::from(u8::from(quad.fades)); 4]);
    let placed = spot.placed();
    batch
        .anchors
        .extend(screens.map(|screen| spot.anchor_at(screen)));
    for screen in screens {
        let (floor, footprint) = spot.floor_at(screen);
        batch.floors.push(floor);
        batch.footprints.push(footprint);
    }
    batch.on_floor |= spot.standing.is_some();
    let (w, h) = size;
    batch.uvs.extend([
        [u0 / w, v0 / h],
        [u1 / w, v0 / h],
        [u1 / w, v1 / h],
        [u0 / w, v1 / h],
    ]);
    let texels = [
        (u0 + 0.5).floor(),
        (v0 + 0.5).floor(),
        (u1 - 0.5).floor(),
        (v1 - 0.5).floor(),
    ];
    batch.sources.extend([texels; 4]);
    batch.quads.push((nearness(placed), first));
    Ok(())
}

/// Builds the sprite and miniature quads of `sector`, the shadows of its statics over `shadows`,
/// the shadow atlas relative to `assets` (none without it), and the pages of its sprites.
pub fn build(
    assets: &Path,
    world: &World,
    sprites: &Sprites,
    shadows: Option<&str>,
    sector: &Sector,
) -> Result<Statics> {
    let dir = assets.join("world");
    let mut objects = Vec::new();
    flatten(&sector.statics, &mut objects);
    // Each sprite of the sector assembled once, then packed into pages.
    let mut atlases = Atlases {
        dir: &dir,
        world,
        loaded: HashMap::new(),
    };
    let mut assembled: BTreeMap<u32, Assembled> = BTreeMap::new();
    for object in &objects {
        if let Some((id, sprite)) = sprite_of(object, sprites)
            && !assembled.contains_key(&id)
            && let Some(image) = assemble(sprite, &mut atlases)?
        {
            assembled.insert(id, image);
        }
    }
    drop(atlases);
    let (pages, placed) = pack(&assembled);
    let shadow_size = shadows
        .map(|path| png_size(&assets.join(path)))
        .transpose()?;
    let mut batches: BTreeMap<BatchKey, Batch> = BTreeMap::new();
    let mut glows: BTreeMap<Option<(buildings::Anchor, u8)>, Glows> = BTreeMap::new();
    let mut sizes: BTreeMap<u32, (f32, f32)> = BTreeMap::new();
    // The size of a quad's texture and the batch's texture.
    let mut texture_of = |texture: Texture| -> Result<((f32, f32), BatchTexture)> {
        Ok(match texture {
            Texture::Atlas(atlas) => {
                let name = world
                    .atlases
                    .get(usize::try_from(atlas)?)
                    .with_context(|| format!("no atlas {atlas}"))?;
                let size = match sizes.get(&atlas) {
                    Some(&size) => size,
                    None => {
                        let size = png_size(&dir.join(name))?;
                        sizes.insert(atlas, size);
                        size
                    }
                };
                (size, BatchTexture::Atlas(asset_path(assets, &dir, name)?))
            }
            Texture::Page(page) => {
                let p = &pages[page];
                ((p.width as f32, p.height as f32), BatchTexture::Page(page))
            }
            Texture::Shadows => (
                shadow_size.context("no shadow atlas")?,
                BatchTexture::Shadows,
            ),
        })
    };
    // The places taken in each cell's chain, by the objects drawn.
    let mut chains: HashMap<u16, u16> = HashMap::new();
    let mut pending = Vec::new();
    let mut drawn = 0;
    for object in &objects {
        let quad = quad(object, &assembled, &placed);
        let shadow = shadows.and_then(|_| shadow_of(object, quad.as_ref()));
        if quad.is_none() && shadow.is_none() {
            continue;
        }
        let place = chains.entry(object.cell).or_default();
        let chain = f32::from(*place);
        if quad.is_some() {
            *place = place.saturating_add(1);
        }
        let cell = &sector.cells[usize::from(object.cell)];
        let (row, column) = (
            usize::from(object.cell) / SIDE,
            usize::from(object.cell) % SIDE,
        );
        let cx = (sector.x as usize * SIDE + column) as f32 + 0.5;
        let cy = (sector.y as usize * SIDE + row) as f32 + 0.5;
        // The centre of the cell in the world and on screen, at height 0 whatever the cell's
        // height: the original orders a list's objects by their cells alone (#112); the depth of
        // the object's draw list, and its layer of the blended things.
        let (back, layer) = if object.type_flags & OVERLAY != 0 {
            (-LIST_4_FORWARD, Layer::Objects)
        } else if object.type_flags & UNDERLAY != 0 {
            // A liquid cell's miniatures go under the surface (`0x62AE90`); the exception of
            // liquid material 13 while the building at (3563, 1336) has the state 2 is not
            // followed.
            if object.flags & MINIATURE != 0 && liquids::liquid(cell) {
                (UNDER_LIQUIDS_BACK, Layer::UnderLiquids)
            } else if object.flags & MINIATURE != 0 || object.mask == 1 {
                (LIST_0_BACK, Layer::Underlays)
            } else {
                (LIST_2_BACK, Layer::LateUnderlays)
            }
        } else {
            (0.0, Layer::Objects)
        };
        let position = Vec2::new(object.x as f32, object.y as f32);
        let part = part_of(object, sector);
        // The shadow: list 1's under list 2, or one drawn right before its sprite in the sprite's
        // list and with its building's part, half a place behind it (`0x62FF60`).
        if let Some((shadow, entry)) = shadow {
            let (spot, shadow_part, shadow_layer) = match entry {
                Entry::List1 => (
                    Spot::new(cx, cy, LIST_1_BACK, chain),
                    None,
                    Layer::StaticShadows,
                ),
                Entry::WithSprite => (Spot::new(cx, cy, back, chain - 0.5), part, Layer::Shadows),
            };
            let (size, texture) = texture_of(Texture::Shadows)?;
            pending.push(Pending {
                key: (Texture::Shadows, shadow_part, shadow_layer),
                texture,
                size,
                quad: shadow,
                position,
                spot,
            });
        }
        let Some(quad) = quad else {
            continue;
        };
        let spot = Spot::new(cx, cy, back, chain).on_floor(object.floor);
        let (size, source) = texture_of(quad.texture)?;
        // A lamp's glow: each corner of its miniature moved out by its size, so three times it
        // across and down, the whole texture over it (`0x62FF60`).
        if object.type_flags & LIGHT != 0
            && let Some(size) = quad.size
        {
            let glow = glows.entry(part).or_insert(Glows {
                part,
                positions: Vec::new(),
                uvs: Vec::new(),
                indices: Vec::new(),
            });
            let first = u32::try_from(glow.positions.len())?;
            let out = [
                Vec2::new(-size, -size),
                Vec2::new(size, -size),
                Vec2::new(size, size),
                Vec2::new(-size, size),
            ];
            for (corner, out) in quad.corners.iter().zip(out) {
                let p =
                    Vec3::from(spot.at(position + *corner + out)) - iso::forward() * GLOW_FORWARD;
                glow.positions.push(p.to_array());
            }
            glow.uvs
                .extend([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
            glow.indices
                .extend([first, first + 2, first + 1, first, first + 3, first + 2]);
        }
        pending.push(Pending {
            key: (quad.texture, part, layer),
            texture: source,
            size,
            quad,
            position,
            spot,
        });
        drawn += 1;
    }
    let floors = stand_on_floors(&mut pending);
    for quad in pending {
        add_quad(&mut batches, quad)?;
    }
    for batch in batches.values_mut() {
        batch.quads.sort_by(|a, b| a.0.total_cmp(&b.0));
        for &(_, first) in &batch.quads {
            batch
                .indices
                .extend([first, first + 2, first + 1, first, first + 3, first + 2]);
        }
    }
    let pages = pages
        .into_iter()
        .map(|page| -> Result<Image> {
            Ok(Image::new(
                Extent3d {
                    width: u32::try_from(page.width)?,
                    height: u32::try_from(page.height.max(1))?,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                if page.pixels.is_empty() {
                    vec![0; 4 * page.width]
                } else {
                    page.pixels
                },
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::RENDER_WORLD,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Statics {
        batches: batches.into_values().collect(),
        pages,
        floors,
        glows: glows.into_values().collect(),
        objects: objects.len(),
        drawn,
    })
}

/// Spawns the glows of a sector, a mesh per building's part and one for the rest, of the
/// glows' material; returns the entities.
pub fn spawn_glows(
    commands: &mut Commands,
    glows: Vec<Glows>,
    meshes: &mut Assets<Mesh>,
    material: &Handle<blended::Sorted>,
) -> Vec<Entity> {
    glows
        .into_iter()
        .map(|glow| {
            let mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, glow.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, glow.uvs)
            .with_inserted_indices(Indices::U32(glow.indices));
            let mut entity =
                commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(material.clone())));
            if let Some((building, mask)) = glow.part {
                let visibility = if mask == buildings::OUTSIDE {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                entity.insert((buildings::Part { building, mask }, visibility));
            }
            entity.id()
        })
        .collect()
}

/// The bit of a type's flags whose sprites fade about the hero, drawn after it: the trees and
/// the walls of type 25, the floors of type 29 (`0x62FF60`, docs/re/drawing.md, "The trees about
/// the heroes").
const FADES: u32 = 0x10;

/// How far about the hero a sprite fades and how opaque it is there: the balance variables
/// `TransSize` and `TransValu` (`0xAD4DF0`, `0xAD4DF4`), 190 pixels at zoom 1 and 50 out of
/// 255 (docs/02 §6.3), by name in `balance.ron`.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct TreeFade {
    pub radius: f32,
    pub alpha: f32,
}

impl TreeFade {
    /// Reads `TransSize` and `TransValu` from `tables/balance.ron`.
    pub fn read(assets: &Path) -> Result<Self> {
        let path = assets.join("tables").join(opensacred_data::tables::BALANCE);
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let balance: opensacred_data::tables::Balance =
            ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        let variable = |name: &str| -> Result<f32> {
            let value = balance
                .f32(name)
                .with_context(|| format!("no {name} in {}", path.display()))?;
            anyhow::ensure!(value.is_finite(), "{name} of {}: {value}", path.display());
            Ok(value)
        };
        Ok(Self {
            radius: variable("TransSize")?,
            alpha: variable("TransValu")?,
        })
    }

    /// The alpha out of 1 that a sprite takes at `offset` from the point it fades about, in the
    /// original's pixels at zoom 1: within the radius across and down, from the alpha at the
    /// point to 1 at the radius' distance, else 1 (`0x62FF60`). As `sprites.wesl` has it, for the
    /// tests.
    #[cfg(test)]
    fn at(&self, offset: Vec2) -> f32 {
        if self.radius <= 0.0 || offset.x.abs() >= self.radius || offset.y.abs() >= self.radius {
            return 1.0;
        }
        let t = (offset.length() / self.radius).min(1.0);
        (self.alpha + (255.0 - self.alpha) * t) / 255.0
    }
}

/// What the sprites fade about, for the sprites' shader (`sprites.wesl`): the hero's point on the
/// original's screen, the centre of its figure, with [`TreeFade`]'s radius and alpha, and its
/// place in the order of drawing, which only the sprites drawn after it fade about. One buffer
/// that every sprite material shares.
#[derive(Resource)]
pub struct Fade {
    pub buffer: Handle<ShaderBuffer>,
    last: [[f32; 4]; 2],
}

impl FromWorld for Fade {
    fn from_world(world: &mut bevy::prelude::World) -> Self {
        let last = [[0.0; 4]; 2];
        let buffer = world
            .resource_mut::<Assets<ShaderBuffer>>()
            .add(ShaderBuffer::new(
                last.to_vec(),
                RenderAssetUsages::default(),
            ));
        Self { buffer, last }
    }
}

/// How high above the hero's feet the centre of its figure stands, in the original's pixels at
/// zoom 1: half its height, about 120 pixels (`iso::MODEL_SCALE`). The client takes the centre
/// of the hero's bounds on the screen (**hypothesis**: the model's).
const HERO_MIDDLE: f32 = 60.0;

/// Keeps the hero's point and place in the fade's buffer.
fn fade(
    heroes: Query<&Transform, With<crate::hero::Body>>,
    size: Option<Res<TreeFade>>,
    mut fade: ResMut<Fade>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
) {
    let (Some(hero), Some(size)) = (heroes.iter().next(), size) else {
        return;
    };
    let at = hero.translation;
    let point = Vec2::new(at.dot(iso::right()), -at.dot(iso::up())) * iso::PIXELS_PER_UNIT
        - Vec2::new(0.0, HERO_MIDDLE);
    let place = crate::models::place_of(at);
    let data = [
        [point.x, point.y, size.radius, size.alpha],
        place.to_array(),
    ];
    if data == fade.last {
        return;
    }
    fade.last = data;
    if let Some(mut buffer) = buffers.get_mut(&fade.buffer) {
        *buffer = ShaderBuffer::new(data.to_vec(), RenderAssetUsages::default());
    }
}

/// What every sector's sprite materials share: the fade's buffer, and the statics' shadows'
/// materials, when there is a shadow atlas.
#[derive(Clone, Copy)]
pub struct Common<'a> {
    pub fade: &'a Fade,
    pub shadows: Option<&'a StaticShadows>,
}

/// The shadow atlas, relative to the asset directory (`SHADOW_TREE00`).
#[derive(Resource)]
pub struct ShadowAtlas(pub String);

/// The materials of the statics' shadows, which every sector's share, so that the night fades
/// them all (`daynight::shadows`): list 1's, and those drawn with their sprites. The original
/// draws them blended by their alpha, filtered linearly, without the alpha test (`0x631EB0`).
/// Those of the sprites on an upper storey's floor have a material of their own ([`Standing`]).
#[derive(Resource)]
pub struct StaticShadows {
    pub list1: Handle<SpriteMaterial>,
    pub with_sprites: Handle<SpriteMaterial>,
    pub on_floors: Handle<SpriteMaterial>,
}

/// Makes the shadows' materials, when the converted textures have the shadow atlas.
fn make_shadows(
    mut commands: Commands,
    atlas: Option<Res<ShadowAtlas>>,
    assets: Res<AssetServer>,
    fade: Res<Fade>,
    mut materials: ResMut<Assets<SpriteMaterial>>,
) {
    let Some(atlas) = atlas else {
        return;
    };
    let texture: Handle<Image> = assets.load(atlas.0.clone());
    let mut material = |layer, on_floor| {
        materials.add(SpriteMaterial {
            atlas: texture.clone(),
            tint: Vec4::ONE,
            fade: fade.buffer.clone(),
            pass: Pass::Fringe(layer),
            alpha_test: false,
            on_floor,
        })
    };
    commands.insert_resource(StaticShadows {
        list1: material(Layer::StaticShadows, false),
        with_sprites: material(Layer::Shadows, false),
        on_floors: material(Layer::Shadows, true),
    });
}

/// Spawns a mesh per atlas and per page and layer, unlit, in its two passes, the shadows' meshes
/// in their one with `shadows`' materials, and the floors that the parts stand on; returns the
/// entities.
pub fn spawn(
    commands: &mut Commands,
    statics: Statics,
    assets: &AssetServer,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<SpriteMaterial>,
    Common { shadows, fade }: Common<'_>,
) -> Vec<Entity> {
    let pages: Vec<Handle<Image>> = statics.pages.into_iter().map(|p| images.add(p)).collect();
    let mut entities: Vec<Entity> = statics
        .floors
        .into_iter()
        .map(|floor| commands.spawn(floor).id())
        .collect();
    for batch in statics.batches {
        // The sprites are tested against the original's reference, the miniatures are not
        // (docs/re/drawing.md, "Blending"); a shadow's texels are all translucent.
        let passes: Vec<Handle<SpriteMaterial>> = match batch.texture {
            BatchTexture::Shadows => match shadows {
                Some(shadows) if batch.layer == Layer::StaticShadows => vec![shadows.list1.clone()],
                Some(shadows) if batch.on_floor => vec![shadows.on_floors.clone()],
                Some(shadows) => vec![shadows.with_sprites.clone()],
                None => continue,
            },
            BatchTexture::Atlas(path) => {
                let atlas = assets.load(path);
                sprite_passes(materials, atlas, fade, batch.layer, false, batch.on_floor)
            }
            BatchTexture::Page(page) => {
                let page = pages[page].clone();
                sprite_passes(materials, page, fade, batch.layer, true, batch.on_floor)
            }
        };
        let mesh = meshes.add(
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, batch.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, batch.uvs)
            .with_inserted_attribute(ATTRIBUTE_ANCHOR, batch.anchors)
            .with_inserted_attribute(ATTRIBUTE_SOURCE, batch.sources)
            .with_inserted_attribute(ATTRIBUTE_SCREEN, batch.screens)
            .with_inserted_attribute(ATTRIBUTE_FADE, batch.fades)
            .with_inserted_attribute(ATTRIBUTE_FLOOR, batch.floors)
            .with_inserted_attribute(ATTRIBUTE_FOOTPRINT, batch.footprints)
            .with_inserted_indices(Indices::U32(batch.indices)),
        );
        for material in passes {
            let mut entity = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material)));
            // A building's parts show while its state is their mask, the shells' outside.
            if let Some((building, mask)) = batch.part {
                let visibility = if mask == buildings::OUTSIDE {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                entity.insert((buildings::Part { building, mask }, visibility));
            }
            entities.push(entity.id());
        }
    }
    entities
}

/// The materials of a batch's two passes over `atlas`, the second in `layer`.
fn sprite_passes(
    materials: &mut Assets<SpriteMaterial>,
    atlas: Handle<Image>,
    fade: &Fade,
    layer: Layer,
    alpha_test: bool,
    on_floor: bool,
) -> Vec<Handle<SpriteMaterial>> {
    [Pass::Body, Pass::Fringe(layer)]
        .into_iter()
        .map(|pass| {
            materials.add(SpriteMaterial {
                atlas: atlas.clone(),
                tint: Vec4::ONE,
                fade: fade.buffer.clone(),
                pass,
                alpha_test,
                on_floor,
            })
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use opensacred_data::world::{Cell, Piece, Sprite};

    use super::*;

    /// The order of drawing's shader library.
    pub(crate) const ORDER: &str = include_str!("order.wesl");

    /// A constant `name` of a shader's `source`: the order of drawing (`order.wesl`), which the
    /// sprites' shader uses, or the models' shader.
    pub(crate) fn shader_constant(source: &str, name: &str) -> f32 {
        let line = source
            .lines()
            .find(|l| l.starts_with(&format!("const {name}: f32 = ")))
            .unwrap();
        line.split('=')
            .nth(1)
            .unwrap()
            .trim()
            .trim_end_matches(';')
            .parse()
            .unwrap()
    }

    /// Writes an RGBA PNG of `width` × `height` pixels, all of `colour`.
    fn write_atlas(path: &Path, width: u32, height: u32, colour: [u8; 4]) {
        let file = fs::File::create(path).unwrap();
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        let pixels: Vec<u8> = (0..width * height).flat_map(|_| colour).collect();
        writer.write_image_data(&pixels).unwrap();
        writer.finish().unwrap();
    }

    #[test]
    fn what_stands_on_a_floor_lies_in_front_of_it() {
        // A floor of the cell (3300, 2500), its plane 112 or 224 units of the models' height high
        // (`+0x33` 4 or 8).
        for height in [112, 224] {
            let spot = Spot::new(3300.5, 2500.5, 0.0, 0.0).on_floor(Some(height));
            let h = f32::from(height) * iso::PIXELS_PER_MODEL_HEIGHT;
            // A model standing anywhere on a cell of the floor around it: its foot on the screen
            // `h` pixels above its ground point, its place its cell's centre after its statics.
            for (cx, cy) in [(3300, 2500), (3298, 2503), (3304, 2497)] {
                let place = iso::ground(cx as f32 + 0.5, cy as f32 + 0.5);
                let model = nearness([place.x, 0.0, place.z, crate::models::PLACE]);
                for (dx, dy) in [(0.0, 0.0), (0.99, 0.0), (0.0, 0.99), (0.99, 0.99)] {
                    let (gx, gy) = (cx as f32 + dx, cy as f32 + dy);
                    let foot = Vec2::new(48.0 * (gx - gy), 24.0 * (gx + gy) - h);
                    let floor = nearness(spot.anchor_at(foot));
                    // In front by more than the half unit a model spreads about its place, of the
                    // floor and of the quads standing on it, however far they come forward.
                    let standing = floor + f64::from(ON_FLOOR + FLOOR_ROOM);
                    assert!(
                        model - 0.5 > standing,
                        "{height} ({gx}, {gy}): {model} {standing}"
                    );
                    // The floor further up the screen, further back on it, lies further back.
                    let above = nearness(spot.anchor_at(foot - Vec2::new(0.0, 24.0)));
                    assert!(above < floor - 30.0, "{height} ({gx}, {gy})");
                }
            }
            // Its anchor is the ground point under the floor's point, `FLOOR_BEHIND` back.
            let corner = Vec2::new(48.0 * 0.3, 24.0 * 5000.0 - h);
            let ground = Vec3::from_array(spot.anchor_at(corner)[..3].try_into().unwrap())
                - iso::forward() * FLOOR_BEHIND;
            let (cx, cy) = (ground.x / iso::CELL, ground.z / iso::CELL);
            assert!((48.0 * (cx - cy) - corner.x).abs() < 1e-2);
            assert!((24.0 * (cx + cy) - (corner.y + h)).abs() < 1e-2);
            assert!(ground.y.abs() < 1e-3);
        }
        // Any other sprite takes its own anchor at every corner.
        let spot = Spot::new(3300.5, 2500.5, 0.0, 3.0);
        assert_eq!(spot.anchor_at(Vec2::new(10.0, 20.0)), spot.placed());
    }

    /// The quads of the house of [`floor_house`]'s part, by their objects.
    const WALL: usize = 0;
    const FLOOR: usize = 1;
    const CARPET: usize = 2;
    const CHEST: usize = 3;
    const FRONT: usize = 4;
    const BED: usize = 5;

    /// A building over the cells (4..=12, 5..=20), its anchor at (8, 8), as the house of sector
    /// 52,39, with its floor 2's parts, of mask 4, and an object outside it, their sprites over
    /// their whole rectangles, built in a directory named after `name`: the statics, and the
    /// height of the floor's plane in pixels.
    fn floor_house(name: &str) -> (Statics, f32) {
        let dir =
            std::env::temp_dir().join(format!("opensacred-game-{name}-{}", std::process::id()));
        fs::create_dir_all(dir.join("world")).unwrap();
        write_atlas(&dir.join("world").join("atlas.png"), 8, 8, [255; 4]);
        let world = World {
            sectors: Vec::new(),
            sheets: Vec::new(),
            atlases: vec!["atlas.png".into()],
        };
        // Sprites over their whole rectangles: a floor, a carpet, a chest, a wall and a bed.
        let sprite = |width, height| {
            Some(Sprite {
                width,
                height,
                pieces: vec![Piece {
                    atlas: 0,
                    source: [0, 0, 8, 8],
                    rect: [0, 0, width, height],
                }],
            })
        };
        let sprites = Sprites {
            sprites: vec![
                None,
                sprite(400, 300),
                sprite(200, 150),
                sprite(60, 60),
                sprite(100, 400),
                sprite(190, 150),
            ],
        };
        // A building over the cells (4..=12, 5..=20), its anchor at (8, 8), as the house of
        // sector 52,39.
        let cell = Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground: 0,
        };
        let mut cells = vec![cell; SIDE * SIDE];
        for cy in 5..=20i8 {
            for cx in 4..=12i8 {
                let at =
                    &mut cells[usize::try_from(cy).unwrap() * SIDE + usize::try_from(cx).unwrap()];
                at.flags = buildings::BUILDING_CELL;
                at.offsets = [8 - cx, 8 - cy];
            }
        }
        // Its floor 2's parts, of mask 4, at `(x, y)` on the screen, and an object outside it.
        let side = u16::try_from(SIDE).unwrap();
        let object = |cx: u16, cy: u16, sprite, (x, y), type_flags, floor| StaticObject {
            cell: cy * side + cx,
            kind: 1,
            flags: BUILDING_PART,
            x,
            y,
            record: 0,
            type_flags,
            mask: 4,
            sprite,
            miniature: None,
            shadow: None,
            floor,
            children: Vec::new(),
        };
        // The floor's plane 140 units high (`+0x33` 5), 240 pixels over the ground.
        let height = 140;
        let h = f32::from(height) * iso::PIXELS_PER_MODEL_HEIGHT;
        let mut outside = object(30, 30, 3, (0, 1440), 9, None);
        outside.flags = 0;
        let sector = Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: vec![
                // A wall of the row before the floor's.
                object(4, 5, 4, (-98, -300), 9, None),
                // The floor, of a cell at the back.
                object(5, 5, 1, (-200, 24), 0x209, Some(height)),
                // A carpet with its top corner at its cell's point on the floor, its art reaching
                // six rows down the screen, as the carpet of that house does.
                object(8, 8, 2, (-100, 168), 9, None),
                // A chest on the carpet, three rows down the screen.
                object(9, 10, 3, (-78, 210), 9, None),
                // A wall at the front corner, which reaches down to the ground.
                object(12, 20, 4, (-434, 452), 9, None),
                // A bed by the cell (9, 9) on its diagonal, drawn after that cell, its art
                // reaching left and three rows down the screen.
                object(10, 8, 5, (-60, 150), 9, None),
                outside,
            ],
            layers: Vec::new(),
            buildings: Vec::new(),
        };
        let statics = build(&dir, &world, &sprites, None, &sector).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        (statics, h)
    }

    /// The depth that the sprites' shader gives quad `q` of `batch` at `p` on the screen, from
    /// its corners, linear across its rectangle: the depth of its place, or the floor's plane's
    /// brought forward over the floor's part, whichever is nearer (`sprites.wesl`).
    fn quad_depth(batch: &Batch, q: usize, p: Vec2) -> f64 {
        let corners = 4 * q..4 * q + 4;
        let s: Vec<Vec2> = batch.screens[corners.clone()]
            .iter()
            .map(|&s| Vec2::from(s))
            .collect();
        let tx = f64::from((p.x - s[0].x) / (s[1].x - s[0].x));
        let ty = f64::from((p.y - s[0].y) / (s[3].y - s[0].y));
        let lerp = |v: Vec<f64>| {
            let top = v[0] + (v[1] - v[0]) * tx;
            let bottom = v[3] + (v[2] - v[3]) * tx;
            top + (bottom - top) * ty
        };
        let own = lerp(
            batch.anchors[corners.clone()]
                .iter()
                .map(|&a| nearness(a))
                .collect(),
        );
        let floor = lerp(
            batch.floors[corners.clone()]
                .iter()
                .map(|&a| nearness(a))
                .collect(),
        );
        let over = (0..2).all(|axis| {
            let f = lerp(
                batch.footprints[corners.clone()]
                    .iter()
                    .map(|f| f64::from(f[axis]))
                    .collect(),
            );
            (0.0..1.0).contains(&f)
        });
        if over { own.max(floor) } else { own }
    }

    /// Whether quad `q` of `batch` covers `p` on the screen.
    fn covers(batch: &Batch, q: usize, p: Vec2) -> bool {
        let [x0, y0] = batch.screens[4 * q];
        let [x1, y1] = batch.screens[4 * q + 2];
        (x0..x1).contains(&p.x) && (y0..y1).contains(&p.y)
    }

    /// Points of a rectangle of the screen, ten steps across and down.
    fn grid(x: f32, y: f32, w: f32, h: f32) -> impl Iterator<Item = Vec2> {
        (0..=10u8).flat_map(move |i| {
            (0..=10u8)
                .map(move |j| Vec2::new(x + w * f32::from(i) / 10.0, y + h * f32::from(j) / 10.0))
        })
    }

    #[test]
    fn what_stands_on_a_floor_lies_over_it_whole() {
        let (statics, h) = floor_house("floor");
        let batch = statics
            .batches
            .iter()
            .find(|b| b.part == Some(((8, 8), 4)))
            .unwrap();
        let other = statics.batches.iter().find(|b| b.part.is_none()).unwrap();
        assert!(batch.on_floor && !other.on_floor);
        let depth = |q, p| quad_depth(batch, q, p);
        let (wall, floor, carpet, chest, front) = (WALL, FLOOR, CARPET, CHEST, FRONT);
        // The carpet over all its rows, and the chest over the carpet, in front of the floor's
        // plane, as the original draws them over it.
        for p in grid(-100.0, 168.0, 200.0, 150.0) {
            assert!(
                depth(carpet, p) > depth(floor, p) + f64::from(ON_FLOOR) - 1e-3,
                "{p}"
            );
        }
        for p in grid(-78.0, 210.0, 60.0, 60.0) {
            assert!(depth(chest, p) > depth(carpet, p), "{p}");
            assert!(depth(chest, p) > depth(floor, p), "{p}");
        }
        // A model on the floor in front of them, at the cell (11, 11), its foot on the floor at
        // its cell's centre, lies in front of them from its head down to its cell's front corner.
        let place = iso::ground(11.5, 11.5);
        let model = nearness([place.x, 0.0, place.z, crate::models::PLACE]) - 0.5;
        let foot = Vec2::new(0.0, 24.0 * 23.0 - h);
        for p in grid(-30.0, foot.y - 120.0, 60.0, 144.0) {
            assert!(model > depth(carpet, p) && model > depth(chest, p), "{p}");
        }
        // Where a sprite does not reach below its cell's point on the floor by half a cell, its
        // place: the carpet's top corner.
        let top = Vec2::new(0.0, 168.0);
        assert_eq!(depth(carpet, top), nearness(batch.anchors[4 * carpet]));
        // The wall before the floor stands on none; the wall at the front comes forward over the
        // floor's part alone, not down to the ground before the building.
        assert!(
            (4 * wall..4 * wall + 4)
                .all(|i| batch.floors[i] == batch.anchors[i] && batch.footprints[i] == [-1.0; 2])
        );
        let ground = Vec2::new(-384.0, 24.0 * 33.0);
        assert_eq!(depth(front, ground), nearness(batch.anchors[4 * front]));
        let edge = Vec2::new(-384.0, 24.0 * 33.0 - h + 20.0);
        assert!(depth(front, edge) > depth(floor, edge));
    }

    /// Where `world` shows on the original's screen at zoom 1.
    fn on_screen(world: Vec3) -> Vec2 {
        let forward = iso::forward();
        let ground = world + forward * (world.y / -forward.y);
        let (cx, cy) = (ground.x / iso::CELL, ground.z / iso::CELL);
        Vec2::new(48.0 * (cx - cy), 24.0 * (cx + cy))
    }

    /// The depth that the models' shader gives a vertex at `world` of a model anchored at
    /// `anchor`, standing on `floor` ([`Floor::under`], zero on none), as [`nearness`] has it
    /// (`models.wesl`).
    fn model_depth(world: Vec3, anchor: Vec4, floor: Vec4) -> f64 {
        let models = include_str!("models.wesl");
        let back = -iso::forward();
        let own = f64::from((world - anchor.truncate()).dot(back));
        let depth = nearness([anchor.x, 0.0, anchor.z, anchor.w])
            + f64::from(shader_constant(models, "SQUEEZE")) * own;
        if floor.w < 0.5 {
            return depth;
        }
        let plane = world + back * ((floor.x - world.y) / back.y);
        let under = Vec3::new(plane.x, 0.0, plane.z) - back * floor.y;
        let on_floor = nearness([under.x, under.y, under.z, 0.0])
            + f64::from(shader_constant(models, "ON_FLOOR_SQUEEZE")) * own;
        depth.max(on_floor)
    }

    #[test]
    fn a_model_on_a_floor_lies_over_it_whole() {
        // On the floor, a model keeps its parts half a unit apart a step of the depth buffer (a
        // thousandth of a unit) apart; one 80 units deep, its root anywhere in its cell, stays in
        // front of the floor's own place in its cell's chain, 32 places at most.
        let squeeze = shader_constant(include_str!("models.wesl"), "ON_FLOOR_SQUEEZE");
        assert!(squeeze * 0.5 >= 0.001);
        assert!(f64::from(ON_FLOOR - squeeze * (40.0 + 33.0)) > CHAIN * 32.0);
        let (statics, h) = floor_house("model-floor");
        let part = ((8, 8), 4);
        let batch = statics
            .batches
            .iter()
            .find(|b| b.part == Some(part))
            .unwrap();
        let floor = statics.floors.iter().find(|f| f.part == part).unwrap();
        // A model on the floor at the cell (9, 9), drawn after the carpet and before the bed of
        // its diagonal, its anchor the centre of its cell at the floor's height.
        let lift = iso::height(h);
        let centre = iso::ground(9.5, 9.5);
        let anchor = Vec4::new(centre.x, lift, centre.z, crate::models::PLACE);
        let on = floor.under(crate::models::place_of(centre));
        assert_eq!((on.x, on.w), (lift, 1.0));
        // Its vertices from a little behind its cell's centre to a cell forward of it, on the
        // floor and above.
        let (mut cut, mut over, mut under) = (0, 0, 0);
        for i in 0..=7u8 {
            for j in 0..=7u8 {
                for up in [0.0, 10.0, 40.0] {
                    let (du, dv) = (f32::from(i) / 5.0 - 0.4, f32::from(j) / 5.0 - 0.4);
                    let world = iso::ground(9.5 + du, 9.5 + dv) + Vec3::Y * (lift + up);
                    let p = on_screen(world);
                    let plane = quad_depth(batch, FLOOR, p);
                    let alone = model_depth(world, anchor, Vec4::ZERO);
                    let depth = model_depth(world, anchor, on);
                    // At its place alone, the floor's plane hides the vertices past its cell's
                    // front corner; standing on the floor, it lies in front of the plane.
                    if alone < plane {
                        cut += 1;
                    }
                    assert!(depth > plane, "({du}, {dv}) {up}: {depth} {plane}");
                    // Where the plane brings it forward, over the carpet drawn before it and under
                    // the bed drawn after it, as the original draws them.
                    if depth > alone {
                        if covers(batch, CARPET, p) {
                            assert!(depth > quad_depth(batch, CARPET, p), "({du}, {dv}) {up}");
                            over += 1;
                        }
                        if covers(batch, BED, p) {
                            assert!(depth < quad_depth(batch, BED, p), "({du}, {dv}) {up}");
                            under += 1;
                        }
                    }
                }
            }
        }
        assert!(cut > 0 && over > 0 && under > 0, "{cut} {over} {under}");
    }

    #[test]
    fn a_buildings_parts_are_found_by_their_flag_and_cell() {
        let cell = |flags| Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags,
            ground: 0,
        };
        let part = |flags, type_flags, mask| StaticObject {
            cell: 0,
            kind: 1,
            flags,
            x: 0,
            y: 0,
            record: 0,
            type_flags,
            mask,
            sprite: 1,
            miniature: None,
            shadow: None,
            floor: None,
            children: Vec::new(),
        };
        let sector = |flags| Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells: vec![cell(flags); SIDE * SIDE],
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: Vec::new(),
        };
        // A room's candle and the shell's wall, of the building at their cell's offsets (none
        // here); a part outside a building, an ordinary static.
        assert_eq!(part_of(&part(0x8, 0x9, 2), &sector(0x3)), Some(((0, 0), 2)));
        assert_eq!(part_of(&part(0x8, 0x9, 1), &sector(0x1)), Some(((0, 0), 1)));
        assert_eq!(part_of(&part(0x8, 0x9, 4), &sector(0x0)), None);
        assert_eq!(part_of(&part(0x0, 0x1_0009, 2), &sector(0x1)), None);
        // In a building's cell the objects of lists 0, 2 and 4 are parts whatever their flags: a
        // slab's miniature of mask 0 under the chapel's stairs, which no state shows (#82), a
        // flower of mask 1 and an object over the sprites; outside a building, none.
        assert_eq!(
            part_of(&part(0x20, 0xC, 0), &sector(0x1)),
            Some(((0, 0), 0))
        );
        assert_eq!(part_of(&part(0x0, 0xD, 1), &sector(0x1)), Some(((0, 0), 1)));
        assert_eq!(
            part_of(&part(0x0, 0x80_0009, 2), &sector(0x1)),
            Some(((0, 0), 2))
        );
        assert_eq!(part_of(&part(0x20, 0xC, 0), &sector(0x0)), None);
    }

    #[test]
    fn a_sprite_is_assembled_from_its_pieces_and_packed_apart() {
        let dir =
            std::env::temp_dir().join(format!("opensacred-game-pieces-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        write_atlas(&dir.join("red.png"), 8, 8, [255, 0, 0, 255]);
        write_atlas(&dir.join("blue.png"), 8, 8, [0, 0, 255, 255]);
        let world = World {
            sectors: Vec::new(),
            sheets: Vec::new(),
            atlases: vec!["red.png".into(), "blue.png".into()],
        };
        // Two pieces side by side from two atlases, and a third below them that runs past the
        // sprite's size, which counts from the pieces' smallest corner.
        let piece = |atlas, source, rect| Piece {
            atlas,
            source,
            rect,
        };
        let sprite = Sprite {
            width: 8,
            height: 5,
            pieces: vec![
                piece(0, [4, 4, 8, 8], [2, 1, 6, 5]),
                piece(1, [0, 0, 4, 4], [6, 1, 10, 5]),
                piece(1, [0, 0, 8, 2], [2, 5, 10, 7]),
            ],
        };
        let mut atlases = Atlases {
            dir: &dir,
            world: &world,
            loaded: HashMap::new(),
        };
        let image = assemble(&sprite, &mut atlases).unwrap().unwrap();
        drop(atlases);
        fs::remove_dir_all(&dir).unwrap();
        // Cut at the size: the third piece's second row goes.
        assert_eq!(
            (image.rect, image.width, image.height),
            ([2, 1, 10, 6], 8, 5)
        );
        assert_eq!(image.pixels.len(), 4 * 8 * 5);
        let pixel = |x: usize, y: usize| &image.pixels[4 * (y * 8 + x)..4 * (y * 8 + x) + 4];
        assert_eq!(pixel(3, 3), [255, 0, 0, 255]);
        assert_eq!(pixel(4, 0), [0, 0, 255, 255]);
        assert_eq!(pixel(7, 4), [0, 0, 255, 255]);
        // Two of them on a page, a gutter apart and whole.
        let sprites = BTreeMap::from([
            (1, image),
            (
                2,
                Assembled {
                    rect: [0, 0, 3, 3],
                    width: 3,
                    height: 3,
                    pixels: vec![7; 4 * 9],
                },
            ),
        ]);
        let (pages, placed) = pack(&sprites);
        assert_eq!(pages.len(), 1);
        let (_, x1, y1) = placed[&1];
        let (_, x2, y2) = placed[&2];
        assert!(x1 + 8 + GUTTER <= x2 || x2 + 3 + GUTTER <= x1 || y1 != y2);
        let page = &pages[0];
        let at = |x: usize, y: usize| {
            &page.pixels[4 * (y * page.width + x)..4 * (y * page.width + x) + 4]
        };
        assert_eq!(at(x1 + 3, y1 + 3), [255, 0, 0, 255]);
        assert_eq!(at(x2 + 2, y2 + 2), [7, 7, 7, 7]);
    }

    #[test]
    fn a_lamps_miniature_glows_three_times_its_size_about_it() {
        let dir = std::env::temp_dir().join(format!("opensacred-game-glow-{}", std::process::id()));
        fs::create_dir_all(dir.join("world")).unwrap();
        write_atlas(&dir.join("world").join("mini.png"), 256, 256, [255; 4]);
        let world = World {
            sectors: Vec::new(),
            sheets: Vec::new(),
            atlases: vec!["mini.png".into()],
        };
        let cell = Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground: 0,
        };
        // Two miniatures of 32 pixels in cell (5, 5): a lamp's and another's.
        let object = |type_flags| StaticObject {
            cell: 5 * u16::try_from(SIDE).unwrap() + 5,
            kind: 1053,
            flags: MINIATURE,
            x: 0,
            y: 240,
            record: 0,
            type_flags,
            mask: 0,
            sprite: 0,
            miniature: Some(opensacred_data::world::Miniature {
                atlas: 0,
                params: [0, 0, 32, 0, 0],
            }),
            shadow: None,
            floor: None,
            children: Vec::new(),
        };
        let sector = Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells: vec![cell; SIDE * SIDE],
            statics: vec![object(LIGHT | 0x8), object(0x8)],
            layers: Vec::new(),
            buildings: Vec::new(),
        };
        let statics = build(
            &dir,
            &world,
            &Sprites {
                sprites: Vec::new(),
            },
            None,
            &sector,
        )
        .unwrap();
        fs::remove_dir_all(&dir).unwrap();
        let [glow] = &statics.glows[..] else {
            panic!("{} glows", statics.glows.len());
        };
        assert_eq!(glow.positions.len(), 4);
        let mini: Vec<Vec3> = statics.batches[0].positions[..4]
            .iter()
            .map(|&p| Vec3::from(p))
            .collect();
        let lit: Vec<Vec3> = glow.positions.iter().map(|&p| Vec3::from(p)).collect();
        let across = |q: &[Vec3]| (q[1] - q[0]).dot(iso::right());
        let centre = |q: &[Vec3]| (q[0] + q[2]) / 2.0;
        assert!((across(&lit) - 3.0 * across(&mini)).abs() < 1e-3);
        let off = centre(&lit) - centre(&mini);
        assert!(off.dot(iso::right()).abs() < 1e-3 && off.dot(iso::up()).abs() < 1e-3);
        // In front of its lamp.
        assert!((off.dot(iso::forward()) + GLOW_FORWARD).abs() < 1e-3);
    }

    #[test]
    fn a_liquid_cells_miniatures_lie_between_the_ground_and_the_surface() {
        let dir =
            std::env::temp_dir().join(format!("opensacred-game-liquid-{}", std::process::id()));
        fs::create_dir_all(dir.join("world")).unwrap();
        write_atlas(&dir.join("world").join("mini.png"), 256, 256, [255; 4]);
        let world = World {
            sectors: Vec::new(),
            sheets: Vec::new(),
            atlases: vec!["mini.png".into()],
        };
        // Water in cell (5, 5), the bank in (6, 5) at height 0.
        let cell = |ground, heights| Cell {
            tile: 0,
            overlays: Vec::new(),
            heights,
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground,
        };
        // An underlay's miniature of `size` pixels with its top left corner at `(x, y)`.
        let object = |column: usize, x, y, size| StaticObject {
            cell: u16::try_from(5 * SIDE + column).unwrap(),
            kind: 1053,
            flags: MINIATURE,
            x,
            y,
            record: 0,
            type_flags: UNDERLAY,
            mask: 0,
            sprite: 0,
            miniature: Some(opensacred_data::world::Miniature {
                atlas: 0,
                params: [0, 0, size, 0, 0],
            }),
            shadow: None,
            floor: None,
            children: Vec::new(),
        };
        // How far behind a vertex along the line of sight a surface lies at the vertex's place on
        // screen: a surface `back` behind a bed at height `bed` pixels. The shader moves a sprite
        // a few units at most.
        let behind =
            |p: Vec3, back: f32, bed: f32| back + (p.y - iso::height(bed)) / -iso::forward().y;
        let shift = 5.0;
        // The water's cell at height 0 and at either end of the heights counted, where the
        // anchors at height 0 take its miniatures furthest from the cell's height.
        for lift in [0_i8, -68, 25] {
            let mut cells = vec![cell(0x40, [0; 4]); SIDE * SIDE];
            cells[5 * SIDE + 5] = cell(0x90, [lift; 4]);
            // The centres of the two cells are at (0, 264 - lift) and (48, 288) on screen. In
            // the water, the tallest reach counted: from 68 pixels above the centre to 139 below;
            // on the bank, 124 below.
            let top = 264 - 68 - i32::from(lift);
            let sector = Sector {
                x: 0,
                y: 0,
                id: 0,
                liquids: [0; 2],
                cells,
                statics: vec![object(5, -103, top, 207), object(6, 0, 288, 124)],
                layers: Vec::new(),
                buildings: Vec::new(),
            };
            let statics = build(
                &dir,
                &world,
                &Sprites {
                    sprites: Vec::new(),
                },
                None,
                &sector,
            )
            .unwrap();
            // Blended with the liquids' layers on either side of the surfaces.
            let layer = |layer| -> Vec<Vec3> {
                let [batch] = &statics
                    .batches
                    .iter()
                    .filter(|b| b.layer == layer)
                    .collect::<Vec<_>>()[..]
                else {
                    panic!("one batch of {layer:?}");
                };
                batch.positions.iter().map(|&p| Vec3::from(p)).collect()
            };
            let (wet, dry) = (layer(Layer::UnderLiquids), layer(Layer::Underlays));
            assert_eq!((wet.len(), dry.len()), (4, 4));
            // A bed up to 61 pixels off the cell's height; the surface flat at height 0
            // (`liquids::build`).
            let lift = f32::from(lift);
            for &p in &wet {
                for bed in [lift - 61.25, lift + 61.25] {
                    assert!(
                        behind(p, iso::GROUND_BEHIND, bed) > shift,
                        "the ground over {p} at {lift}"
                    );
                }
                assert!(
                    behind(p, liquids::BACK, 0.0) < -shift,
                    "the water under {p} at {lift}"
                );
            }
            for &p in &dry {
                assert!(
                    behind(p, liquids::BACK, 0.0) > shift,
                    "the water over {p} at {lift}"
                );
            }
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sprites_at_one_depth_come_forward_in_the_original_order() {
        // An atlas of 8 × 8 pixels, and a sprite of one piece of it.
        let dir =
            std::env::temp_dir().join(format!("opensacred-game-order-{}", std::process::id()));
        fs::create_dir_all(dir.join("world")).unwrap();
        write_atlas(&dir.join("world").join("atlas.png"), 8, 8, [255; 4]);
        let world = World {
            sectors: Vec::new(),
            sheets: Vec::new(),
            atlases: vec!["atlas.png".into()],
        };
        let piece = Piece {
            atlas: 0,
            source: [0, 0, 8, 8],
            rect: [0, 0, 8, 8],
        };
        let sprites = Sprites {
            sprites: vec![
                None,
                Some(Sprite {
                    width: 8,
                    height: 8,
                    pieces: vec![piece],
                }),
            ],
        };
        let cell = Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground: 0,
        };
        let object = |column: u16, row: u16| StaticObject {
            cell: row * u16::try_from(SIDE).unwrap() + column,
            kind: 1,
            flags: 0,
            x: 48 * (i32::from(column) - i32::from(row)),
            y: 24 * (i32::from(column) + i32::from(row)),
            record: 0,
            type_flags: 0,
            mask: 0,
            sprite: 1,
            miniature: None,
            shadow: None,
            floor: None,
            children: Vec::new(),
        };
        // Two objects in the chain of cell (5, 5), one in the next cell of its diagonal, and one
        // of the diagonal behind. The cell (5, 5) lies 5 pixels higher than the next, as the
        // chapel's wall beside its cabinet (#112), and the cell behind as high as a cell goes:
        // neither height changes the order.
        let mut cells = vec![cell; SIDE * SIDE];
        cells[5 * SIDE + 5].heights = [5; 4];
        cells[4 * SIDE + 4].heights = [i8::MAX; 4];
        let sector = Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: vec![object(5, 5), object(5, 5), object(6, 4), object(4, 4)],
            layers: Vec::new(),
            buildings: Vec::new(),
        };
        let statics = build(&dir, &world, &sprites, None, &sector).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        let batch = &statics.batches[0];
        let anchors: Vec<[f32; 4]> = batch.anchors.iter().step_by(4).copied().collect();
        let [first, second, next, behind] = anchors[..] else {
            panic!("{anchors:?}");
        };
        // Its quads go far to near, the one behind first.
        let quads: Vec<u32> = batch.indices.chunks(6).map(|q| q[0] / 4).collect();
        assert_eq!(quads, [3, 0, 1, 2]);
        assert!(nearness(behind) < nearness(first) && nearness(second) < nearness(next));
        for (name, value) in [("ACROSS", ACROSS), ("CHAIN", CHAIN)] {
            let shader = f64::from(shader_constant(ORDER, name));
            assert!(
                (shader - value).abs() < 1e-9,
                "{name}: {shader} in the shader"
            );
        }
        let at = |a: [f32; 4]| Vec3::new(a[0], a[1], a[2]);
        // In one cell, the same anchor, and the place in the chain.
        assert_eq!(at(first), at(second));
        assert_eq!((first[3], second[3]), (0.0, 1.0));
        // Along a diagonal, the same depth, and the cell of greater cx further right.
        assert_eq!(next[3], 0.0);
        let between = at(next) - at(first);
        assert!(between.dot(iso::forward()).abs() < 1e-2);
        let step = between.dot(iso::right());
        assert!(step > 70.0);
        // The 32 places of the longest chain stay within the step to the next cell.
        assert!(31.0 * shader_constant(ORDER, "CHAIN") < shader_constant(ORDER, "ACROSS") * step);
        assert_eq!(shader_constant(ORDER, "DISTANCE"), iso::DISTANCE);
    }

    #[test]
    fn a_tree_fades_about_the_hero() {
        // `TransSize` and `TransValu` of the balance table.
        let dir = std::env::temp_dir().join(format!("opensacred-game-fade-{}", std::process::id()));
        fs::create_dir_all(dir.join("tables")).unwrap();
        let variable = |name: &str, value: f32| opensacred_data::tables::BalanceVariable {
            name: name.to_owned(),
            offset: 0,
            value: opensacred_data::tables::BalanceValue::F32(value),
        };
        let balance = opensacred_data::tables::Balance {
            variables: vec![variable("TransSize", 190.0), variable("TransValu", 50.0)],
            ..Default::default()
        };
        fs::write(
            dir.join("tables").join(opensacred_data::tables::BALANCE),
            ron::to_string(&balance).unwrap(),
        )
        .unwrap();
        let size = TreeFade::read(&dir).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            size,
            TreeFade {
                radius: 190.0,
                alpha: 50.0
            }
        );
        let alpha = |x: f32, y: f32| (size.at(Vec2::new(x, y)) * 255.0).round();
        // 50 out of 255 at the hero, half way at 95 pixels, whole at 190 and beyond, and past the
        // radius' distance within its box.
        assert_eq!(alpha(0.0, 0.0), 50.0);
        assert_eq!(alpha(95.0, 0.0), 153.0);
        assert_eq!(alpha(0.0, -190.0), 255.0);
        assert_eq!(alpha(150.0, 150.0), 255.0);
        // So the original's alpha test drops the faded texels within 13 pixels of the hero.
        assert!(size.at(Vec2::new(13.0, 0.0)) * 255.0 < 64.5);
        assert!(size.at(Vec2::new(14.0, 0.0)) * 255.0 > 64.5);
    }

    #[test]
    fn abutting_sprites_meet_on_the_screen() {
        let dir = std::env::temp_dir().join(format!("opensacred-game-abut-{}", std::process::id()));
        fs::create_dir_all(dir.join("world")).unwrap();
        write_atlas(&dir.join("world").join("atlas.png"), 8, 8, [255; 4]);
        let world = World {
            sectors: Vec::new(),
            sheets: Vec::new(),
            atlases: vec!["atlas.png".into()],
        };
        let sprites = Sprites {
            sprites: vec![
                None,
                Some(Sprite {
                    width: 8,
                    height: 8,
                    pieces: vec![Piece {
                        atlas: 0,
                        source: [0, 0, 8, 8],
                        rect: [0, 0, 8, 8],
                    }],
                }),
            ],
        };
        let cell = Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground: 0,
        };
        // Two sprites of cells far apart in the start's sector, the second's left edge on the
        // first's right edge, where the chapel's two floors meet (#113).
        let object = |cell: u16, x: i32| StaticObject {
            cell,
            kind: 1,
            flags: 0,
            x,
            y: 137_827,
            record: 0,
            type_flags: 0,
            mask: 0,
            sprite: 1,
            miniature: None,
            shadow: None,
            floor: None,
            children: Vec::new(),
        };
        let side = u16::try_from(SIDE).unwrap();
        let sector = Sector {
            x: 50,
            y: 39,
            id: 0,
            liquids: [0; 2],
            cells: vec![cell; SIDE * SIDE],
            statics: vec![object(5 * side + 5, 34_806), object(side + 9, 34_814)],
            layers: Vec::new(),
            buildings: Vec::new(),
        };
        let statics = build(&dir, &world, &sprites, None, &sector).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        let screens = &statics.batches[0].screens;
        // Corners top-left, top-right, bottom-right, bottom-left: the first's right edge is the
        // second's left edge, to the bit.
        assert_eq!((screens[1], screens[2]), (screens[4], screens[7]));
        assert_eq!(screens[4], [34_814.0, 137_827.0]);
        let shader = shader_constant(include_str!("sprites.wesl"), "PIXELS_PER_UNIT");
        assert!((shader - iso::PIXELS_PER_UNIT).abs() < 1e-6);
    }

    #[test]
    fn a_shadow_is_a_rectangle_or_leans_right_over_its_cell() {
        // `Bench 2`'s: cell 74, the foot at (78, 100), 80 tall, a rectangle.
        let bench = shadow_quad(&Shadow {
            cell: 74,
            foot: [78, 100],
            size: 80,
            leaning: false,
        });
        assert_eq!(
            bench.corners,
            [
                Vec2::new(-2.0, 20.0),
                Vec2::new(158.0, 20.0),
                Vec2::new(158.0, 100.0),
                Vec2::new(-2.0, 100.0),
            ]
        );
        // Row 4, column 10 of the atlas's 32-texel cells.
        assert_eq!(bench.source, [320.0, 128.0, 352.0, 160.0]);
        // `Tree17`'s: cell 2, the foot at (190, 440), 240 tall, its top edge shifted right by 240.
        let tree = shadow_quad(&Shadow {
            cell: 2,
            foot: [190, 440],
            size: 240,
            leaning: true,
        });
        assert_eq!(
            tree.corners,
            [
                Vec2::new(190.0, 200.0),
                Vec2::new(670.0, 200.0),
                Vec2::new(430.0, 440.0),
                Vec2::new(-50.0, 440.0),
            ]
        );
        assert_eq!(tree.source, [64.0, 0.0, 96.0, 32.0]);
    }

    #[test]
    fn a_shadow_goes_to_list_1_or_with_its_sprite() {
        let dir =
            std::env::temp_dir().join(format!("opensacred-game-shadows-{}", std::process::id()));
        fs::create_dir_all(dir.join("world")).unwrap();
        write_atlas(&dir.join("world").join("atlas.png"), 8, 8, [255; 4]);
        write_atlas(&dir.join("shadows.png"), 512, 512, [0, 0, 0, 170]);
        let world = World {
            sectors: Vec::new(),
            sheets: Vec::new(),
            atlases: vec!["atlas.png".into()],
        };
        let sprites = Sprites {
            sprites: vec![
                None,
                Some(Sprite {
                    width: 8,
                    height: 8,
                    pieces: vec![Piece {
                        atlas: 0,
                        source: [0, 0, 8, 8],
                        rect: [0, 0, 8, 8],
                    }],
                }),
            ],
        };
        let cell = Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground: 0,
        };
        let shadow = Shadow {
            cell: 74,
            foot: [4, 8],
            size: 8,
            leaning: false,
        };
        // In cell (5, 5): a bench of mask 1 without a sprite, one of mask 2 with its sprite, one
        // without a shadow by its flag, and one of mask 0.
        let object = |mask, sprite, flags| StaticObject {
            cell: 5 * u16::try_from(SIDE).unwrap() + 5,
            kind: 9181,
            flags,
            x: 0,
            y: 264,
            record: 0,
            type_flags: CASTS_SHADOW | 9,
            mask,
            sprite,
            miniature: None,
            shadow: Some(shadow),
            floor: None,
            children: Vec::new(),
        };
        let sector = Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells: vec![cell; SIDE * SIDE],
            statics: vec![
                object(1, 0, 0),
                object(2, 1, 0),
                object(1, 1, NO_SHADOW),
                object(0, 1, 0),
            ],
            layers: Vec::new(),
            buildings: Vec::new(),
        };
        let statics = build(&dir, &world, &sprites, Some("shadows.png"), &sector).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        let shadows: Vec<&Batch> = statics
            .batches
            .iter()
            .filter(|b| matches!(b.texture, BatchTexture::Shadows))
            .collect();
        let [list1, with_sprite] = shadows[..] else {
            panic!("two batches of shadows");
        };
        assert_eq!(
            (list1.layer, with_sprite.layer),
            (Layer::StaticShadows, Layer::Shadows)
        );
        assert_eq!((list1.quads.len(), with_sprite.quads.len()), (1, 1));
        // List 1's in place 0 of the cell, which the first static, without a sprite, leaves to
        // the second's sprite; that one's shadow half a place before its sprite.
        let sprite = statics
            .batches
            .iter()
            .find(|b| matches!(b.texture, BatchTexture::Page(_)))
            .unwrap();
        assert_eq!(
            (list1.anchors[0][3], with_sprite.anchors[0][3]),
            (0.0, -0.5)
        );
        assert_eq!(sprite.anchors[0][3], 0.0);
        assert_eq!(
            Vec3::from_slice(&with_sprite.anchors[0]),
            Vec3::from_slice(&sprite.anchors[0])
        );
        // List 1's depth.
        let back = |b: &Batch| -Vec3::from_slice(&b.anchors[0]).dot(-iso::forward());
        let ground = -iso::ground(5.5, 5.5).dot(-iso::forward());
        assert!((back(list1) - ground - LIST_1_BACK).abs() < 1e-2);
        // The rectangle about the foot, over the whole of its cell.
        assert_eq!(
            list1.screens[..4],
            [[-4.0, 264.0], [12.0, 264.0], [12.0, 272.0], [-4.0, 272.0]]
        );
        assert_eq!(list1.uvs[0], [320.0 / 512.0, 128.0 / 512.0]);
    }
}
