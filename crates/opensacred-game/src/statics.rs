//! The static objects of a sector (docs/02 §6.1): each object's sprite, assembled from pieces of
//! atlases, drawn with its top-left corner at the object's screen position. The sector's sprites
//! are assembled once when it is built and packed into pages, so that filtering runs across the
//! pieces' edges as within them.
//!
//! The original draws the ground, then the objects back to front. Here a sprite is a quad in the
//! plane of the screen, as flat as the original's, set at the depth of the centre of its cell,
//! so that the depth buffer orders the sprites and the hero among themselves; the ground lies
//! behind all of them (`terrain`). Sprites at the same depth, those of one cell and those of the
//! cells of one diagonal at one height, would fight; the shader brings each a little forward in
//! the original's order of drawing (`sprites.wesl`). The sprites of one page share one mesh.
//! Miniatures (flag 0x20) are squares cut from their type's atlas, the miniatures of one atlas
//! one mesh; the animated ones (flag 0x40) show their first frame, still. A light source's
//! miniature (type flag `0x20000`, the lamps) carries a glow, a quad of `PARTICLE_GLOW01` three
//! times its size about it, which `daynight` shows by night (`0x62FF60`, docs/re/daynight.md).

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::mesh::{
    Indices, MeshVertexAttribute, MeshVertexBufferLayoutRef, PrimitiveTopology, VertexFormat,
};
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::prelude::{
    AlphaMode, App, Asset, AssetServer, Assets, Commands, Entity, Handle, Image, Mesh, Mesh3d,
    MeshMaterial3d, Plugin, Resource, StandardMaterial, TypePath, Vec2, Vec3, Visibility,
};
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, RenderPipelineDescriptor, SpecializedMeshPipelineError,
    TextureDimension, TextureFormat,
};
use bevy::shader::ShaderRef;
use opensacred_data::world::{SIDE, Sector, Sprite, Sprites, StaticObject, World};

use crate::terrain::{Rgba, read_png};
use crate::{buildings, iso};

/// The texture of a batch: an atlas, relative to the asset directory, or a page of the sector's
/// sprites.
enum BatchTexture {
    Atlas(String),
    Page(usize),
}

/// The quads of one atlas or page.
struct Batch {
    texture: BatchTexture,
    /// Parts of a building's: the building and their mask (see [`part_of`]).
    part: Option<(buildings::Anchor, u8)>,
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    anchors: Vec<[f32; 4]>,
    sources: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

/// What places a vertex's sprite in depth (`sprites.wesl`): the centre of its cell at its
/// height and its draw list's depth, and its object's place in the cell's chain.
pub const ATTRIBUTE_ANCHOR: MeshVertexAttribute = MeshVertexAttribute::new(
    "SpriteAnchor",
    0x5350_5249_5445_4143,
    VertexFormat::Float32x4,
);

/// The rectangle of the atlas that a vertex's piece may sample, in texture coordinates: its
/// source half a texel in from each edge (`sprites.wesl`).
pub const ATTRIBUTE_SOURCE: MeshVertexAttribute = MeshVertexAttribute::new(
    "SpriteSource",
    0x5350_5249_5445_5352,
    VertexFormat::Float32x4,
);

/// The shader of [`SpriteMaterial`], embedded in the binary.
const SHADER: &str = "embedded://opensacred_game/sprites.wesl";

/// The material of the sprites of one atlas: unlit, its edges through alpha to coverage, each
/// sprite brought a little forward in the original's order of drawing (`sprites.wesl`).
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct SpriteMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub atlas: Handle<Image>,
}

impl Material for SpriteMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::AlphaToCoverage
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
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            ATTRIBUTE_ANCHOR.at_shader_location(2),
            ATTRIBUTE_SOURCE.at_shader_location(3),
        ])?];
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// The sprites' material and its shader.
pub struct StaticsPlugin;

impl Plugin for StaticsPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "sprites.wesl");
        app.add_plugins(MaterialPlugin::<SpriteMaterial>::default());
    }
}

/// The sprite meshes of a sector, built on the CPU, and the pages of its sprites.
#[derive(Default)]
pub struct Statics {
    batches: Vec<Batch>,
    pages: Vec<Image>,
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
pub struct Glow(pub Handle<StandardMaterial>);

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
/// How far back along the line of sight the objects of lists 0 and 2 go: under the sprites of
/// list 3 and the hero, above the ground, their order within a list unchanged.
pub const LIST_0_BACK: f32 = 4_000.0;
pub const LIST_2_BACK: f32 = 3_000.0;
/// How far forward the objects of list 4 come: over the sprites of list 3 around them. Whether
/// the original draws them over the characters too is not known.
const LIST_4_FORWARD: f32 = 1_000.0;

/// The width of a page of a sector's sprites, and the most it grows to before another begins.
const PAGE: usize = 2048;
/// Transparent texels between two sprites of a page.
const GUTTER: usize = 1;

/// Where a quad's texels come from: a miniature's atlas, or a page of the sector's sprites.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Texture {
    Atlas(u32),
    Page(usize),
}

/// A quad to draw: its texture, the corners on screen (top-left, top-right, bottom-right,
/// bottom-left) relative to the object's position, and the rectangle of the texture in pixels.
struct Quad {
    texture: Texture,
    corners: [Vec2; 4],
    source: [f32; 4],
    /// A miniature's size across on screen, by which its glow reaches past it.
    size: Option<f32>,
}

/// A sprite assembled from its pieces, one to one, over the rectangle the pieces cover (they may
/// run past the sprite's size): so that filtering blends the texels on either side of a piece's
/// edge as it does inside a piece. Filtering each piece in its own atlas could not reach the
/// next piece's texels, and the step between two pieces showed as a line once magnified.
struct Assembled {
    /// The rectangle in the sprite's pixels: `x0, y0, x1, y1`.
    rect: [i32; 4],
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

/// The sprite's pieces copied from their atlases into one image; `None` without pieces.
fn assemble(sprite: &Sprite, atlases: &mut Atlases<'_>) -> Result<Option<Assembled>> {
    let mut rect: Option<[i32; 4]> = None;
    for piece in &sprite.pieces {
        let [x0, y0, x1, y1] = piece.rect.map(i32::from);
        rect = Some(match rect {
            None => [x0, y0, x1, y1],
            Some([a, b, c, d]) => [a.min(x0), b.min(y0), c.max(x1), d.max(y1)],
        });
    }
    let Some(rect) = rect else {
        return Ok(None);
    };
    let width = usize::try_from(rect[2] - rect[0])?;
    let height = usize::try_from(rect[3] - rect[1])?;
    let mut pixels = vec![0u8; 4 * width * height];
    for piece in &sprite.pieces {
        let atlas = atlases.get(piece.atlas)?;
        let [u0, v0, u1, v1] = piece.source.map(usize::from);
        let x0 = usize::try_from(i32::from(piece.rect[0]) - rect[0])?;
        let y0 = usize::try_from(i32::from(piece.rect[1]) - rect[1])?;
        for v in v0..v1.min(atlas.height) {
            let from = 4 * (v * atlas.width + u0);
            let to = 4 * (v * atlas.width + u1.min(atlas.width));
            let at = 4 * ((y0 + v - v0) * width + x0);
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
    })
}

/// Builds the sprite and miniature quads of `sector`, and the pages of its sprites.
pub fn build(assets: &Path, world: &World, sprites: &Sprites, sector: &Sector) -> Result<Statics> {
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
    let right = iso::right();
    let up = iso::up();
    let mut batches: BTreeMap<(Texture, Option<(buildings::Anchor, u8)>), Batch> = BTreeMap::new();
    let mut glows: BTreeMap<Option<(buildings::Anchor, u8)>, Glows> = BTreeMap::new();
    let mut sizes: BTreeMap<u32, (f32, f32)> = BTreeMap::new();
    // The places taken in each cell's chain, by the objects drawn.
    let mut chains: HashMap<u16, u16> = HashMap::new();
    let mut drawn = 0;
    for object in &objects {
        let Some(quad) = quad(object, &assembled, &placed) else {
            continue;
        };
        let place = chains.entry(object.cell).or_default();
        let chain = f32::from(*place);
        *place = place.saturating_add(1);
        let cell = &sector.cells[usize::from(object.cell)];
        let (row, column) = (
            usize::from(object.cell) / SIDE,
            usize::from(object.cell) % SIDE,
        );
        let cx = (sector.x as usize * SIDE + column) as f32 + 0.5;
        let cy = (sector.y as usize * SIDE + row) as f32 + 0.5;
        let lift = cell.heights.iter().map(|&h| f32::from(h)).sum::<f32>() / 4.0;
        // The centre of the cell in the world and on screen; the depth of the object's draw list.
        let back = if object.type_flags & OVERLAY != 0 {
            -LIST_4_FORWARD
        } else if object.type_flags & UNDERLAY != 0 {
            if object.flags & MINIATURE != 0 || object.mask == 1 {
                LIST_0_BACK
            } else {
                LIST_2_BACK
            }
        } else {
            0.0
        };
        let anchor = iso::ground(cx, cy) + Vec3::Y * iso::height(lift) + iso::forward() * back;
        let placed_at = [anchor.x, anchor.y, anchor.z, chain];
        let anchor_screen = Vec2::new(48.0 * (cx - cy), 24.0 * (cx + cy) - lift);
        let position = Vec2::new(object.x as f32, object.y as f32);
        let at = |corner: Vec2| -> [f32; 3] {
            let screen = position + corner;
            let p = anchor
                + right * ((screen.x - anchor_screen.x) / iso::PIXELS_PER_UNIT)
                + up * ((anchor_screen.y - screen.y) / iso::PIXELS_PER_UNIT);
            p.to_array()
        };
        let (size, source) = match quad.texture {
            Texture::Atlas(atlas) => {
                let size = match sizes.get(&atlas) {
                    Some(&size) => size,
                    None => {
                        let name = world
                            .atlases
                            .get(usize::try_from(atlas)?)
                            .with_context(|| format!("no atlas {atlas}"))?;
                        let size = png_size(&dir.join(name))?;
                        sizes.insert(atlas, size);
                        size
                    }
                };
                let name = &world.atlases[usize::try_from(atlas)?];
                (size, BatchTexture::Atlas(asset_path(assets, &dir, name)?))
            }
            Texture::Page(page) => {
                let p = &pages[page];
                ((p.width as f32, p.height as f32), BatchTexture::Page(page))
            }
        };
        let part = part_of(object, sector);
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
                let p = Vec3::from(at(*corner + out)) - iso::forward() * GLOW_FORWARD;
                glow.positions.push(p.to_array());
            }
            glow.uvs
                .extend([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
            glow.indices
                .extend([first, first + 2, first + 1, first, first + 3, first + 2]);
        }
        let batch = batches.entry((quad.texture, part)).or_insert(Batch {
            texture: source,
            part,
            positions: Vec::new(),
            uvs: Vec::new(),
            anchors: Vec::new(),
            sources: Vec::new(),
            indices: Vec::new(),
        });
        // The texels map onto the quad one to one; the shader keeps the samples half a texel in
        // from the edges, where filtering would blend in the texels around them.
        let [u0, v0, u1, v1] = quad.source;
        let first = u32::try_from(batch.positions.len())?;
        batch.positions.extend(quad.corners.map(at));
        batch.anchors.extend([placed_at; 4]);
        let (w, h) = size;
        batch.uvs.extend([
            [u0 / w, v0 / h],
            [u1 / w, v0 / h],
            [u1 / w, v1 / h],
            [u0 / w, v1 / h],
        ]);
        let inside = [
            (u0 + 0.5) / w,
            (v0 + 0.5) / h,
            (u1 - 0.5) / w,
            (v1 - 0.5) / h,
        ];
        batch.sources.extend([inside; 4]);
        batch
            .indices
            .extend([first, first + 2, first + 1, first, first + 3, first + 2]);
        drawn += 1;
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
    material: &Handle<StandardMaterial>,
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

/// Spawns a mesh per atlas and per page, unlit, its edges through alpha to coverage; returns the
/// entities.
pub fn spawn(
    commands: &mut Commands,
    statics: Statics,
    assets: &AssetServer,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<SpriteMaterial>,
) -> Vec<Entity> {
    let pages: Vec<Handle<Image>> = statics.pages.into_iter().map(|p| images.add(p)).collect();
    let mut entities = Vec::new();
    for batch in statics.batches {
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, batch.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, batch.uvs)
        .with_inserted_attribute(ATTRIBUTE_ANCHOR, batch.anchors)
        .with_inserted_attribute(ATTRIBUTE_SOURCE, batch.sources)
        .with_inserted_indices(Indices::U32(batch.indices));
        let atlas = match batch.texture {
            BatchTexture::Atlas(path) => assets.load(path),
            BatchTexture::Page(page) => pages[page].clone(),
        };
        let material = SpriteMaterial { atlas };
        let mut entity = commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(material)),
        ));
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
    entities
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
        // Two pieces side by side from two atlases, and a third that runs past the sprite's
        // size, below them.
        let piece = |atlas, source, rect| Piece {
            atlas,
            source,
            rect,
        };
        let sprite = Sprite {
            width: 8,
            height: 4,
            pieces: vec![
                piece(0, [4, 4, 8, 8], [0, 0, 4, 4]),
                piece(1, [0, 0, 4, 4], [4, 0, 8, 4]),
                piece(1, [0, 0, 8, 2], [0, 4, 8, 6]),
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
        assert_eq!(
            (image.rect, image.width, image.height),
            ([0, 0, 8, 6], 8, 6)
        );
        let pixel = |x: usize, y: usize| &image.pixels[4 * (y * 8 + x)..4 * (y * 8 + x) + 4];
        assert_eq!(pixel(3, 3), [255, 0, 0, 255]);
        assert_eq!(pixel(4, 0), [0, 0, 255, 255]);
        assert_eq!(pixel(7, 5), [0, 0, 255, 255]);
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
        };
        let statics = build(
            &dir,
            &world,
            &Sprites {
                sprites: Vec::new(),
            },
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
            children: Vec::new(),
        };
        // Two objects in the chain of cell (5, 5), one in the next cell of its diagonal.
        let sector = Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells: vec![cell; SIDE * SIDE],
            statics: vec![object(5, 5), object(5, 5), object(6, 4)],
            layers: Vec::new(),
        };
        let statics = build(&dir, &world, &sprites, &sector).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        let anchors: Vec<[f32; 4]> = statics.batches[0]
            .anchors
            .iter()
            .step_by(4)
            .copied()
            .collect();
        let [first, second, next] = anchors[..] else {
            panic!("{anchors:?}");
        };
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
}
