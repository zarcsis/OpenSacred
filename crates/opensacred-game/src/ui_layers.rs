//! The interface's layers (#2636): the pieces of the interface that the original draws at
//! 1024 × 768 one to one are assembled at the original's pixels, one to one, by cameras of their
//! own into pictures, and each picture is drawn in the window, scaled and filtered once. Drawn
//! each on its own, scaled by a fraction and filtered, the pieces left lines at their joins
//! (#2632, #2636), and the maintainer's rule is to filter only what is assembled (2026-11-12).
//!
//! A piece is an image or a background of the interface. The nodes stay where they are: their
//! layout, their pointer's events, their texts and their colours are the window's, and the
//! modules that set them see their own. What draws at the window's pixels still draws there, in
//! the order the interface stacks it: a text's glyphs, a material's node ([`Native`]: the heroes'
//! figures, the glows), a camera's picture (the items' models, the menus' water), an image that a
//! band would show less of ([`sharper_apart`]: the compass's needle, the day's disc), a border, a
//! shadow, a gradient. So each stacking context (a root, or a subtree raised by a `GlobalZIndex`
//! of its own) is cut into bands at what draws at the window's pixels, and each band of two
//! pieces or more takes a picture: a mirror of its root's tree under its camera, laid out by the
//! same styles at the original's pixels, draws the band's pieces, and its picture is drawn in the
//! window where the band's first piece stacks ([`composites`]). The window's own drawing of those
//! pieces is left out as Bevy extracts the interface ([`hide`]), from a band's second frame on:
//! in its first its camera draws its first picture.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::marker::PhantomData;

use bevy::asset::{AssetId, embedded_asset};
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{CameraUpdateSystems, CompositingSpace, ImageRenderTarget, RenderTarget};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendState, RenderPipelineDescriptor, TextureFormat,
};
use bevy::render::sync_world::MainEntity;
use bevy::render::{Extract, ExtractSchedule, RenderApp};
use bevy::shader::ShaderRef;
use bevy::ui::{
    BackgroundGradient, BorderGradient, ComputedNode, UiGlobalTransform, UiSystems, UiTargetCamera,
};
use bevy::ui_render::prelude::{MaterialNode, UiMaterial, UiMaterialPlugin};
use bevy::ui_render::ui_material::UiMaterialKey;
use bevy::ui_render::{ExtractedUiItem, ExtractedUiNodes, NodeType, RenderUiSystems};
use bevy::window::PrimaryWindow;

/// The shader of [`Picture`], embedded in the binary.
const SHADER: &str = "embedded://opensacred_game/layer.wesl";

/// The pieces a band must draw for a picture of its own: one alone has no joins.
const LEAST_PIECES: usize = 2;

/// The layers' cameras' order, before the world's (0): each picture is drawn in the frame that
/// assembles it.
const ORDER: isize = -100;

/// The render layer of the layers' cameras, which no entity of the world is on: they draw the
/// interface alone.
const LAYER: usize = 31;

/// A node of the interface that draws at the window's pixels where the interface stacks it,
/// between the bands: a material's, made [`Native`] by [`NativeMaterialPlugin`].
#[derive(Component)]
pub struct Native;

/// Bevy's plugin of the interface's material `M`, whose nodes draw at the window's pixels
/// ([`Native`]): every material of the interface is added through it.
pub struct NativeMaterialPlugin<M>(PhantomData<M>);

impl<M> Default for NativeMaterialPlugin<M> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<M: UiMaterial> Plugin for NativeMaterialPlugin<M>
where
    M::Data: PartialEq + Eq + Hash + Clone,
{
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<M>::default())
            .add_observer(native_material::<M>);
    }
}

/// Makes a node of the material `M` [`Native`].
fn native_material<M: UiMaterial>(added: On<Add<MaterialNode<M>>>, mut commands: Commands) {
    commands.entity(added.entity).insert(Native);
}

/// A band's picture on the window: its sRGB values, premultiplied (`layer.wesl`).
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
struct Picture {
    #[texture(0)]
    picture: Handle<Image>,
}

impl UiMaterial for Picture {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn specialize(descriptor: &mut RenderPipelineDescriptor, _key: UiMaterialKey<Self>) {
        let target = descriptor
            .fragment
            .as_mut()
            .and_then(|fragment| fragment.targets.first_mut())
            .and_then(Option::as_mut);
        if let Some(target) = target {
            target.blend = Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        }
    }
}

/// The node in the window that draws a band's picture.
#[derive(Component)]
struct Composite;

/// A node of a band's mirror.
#[derive(Component)]
struct Mirror;

/// A band's parts: its camera, its picture and the material that draws it, the frame its
/// mirror lies in (the window's size in the original's pixels, under the camera), the node that
/// shows the picture in the window, the window's size the picture was made for, and the mirror
/// of each node of its root's tree.
struct Parts {
    camera: Entity,
    picture: Handle<Image>,
    material: Handle<Picture>,
    frame: Entity,
    composite: Entity,
    whole: Vec2,
    mirrors: HashMap<Entity, Entity>,
}

/// A band: the root of its stacking context's tree, the piece the interface stacks first, its
/// pieces, its parts, and whether its parts were made this frame: its camera draws its first
/// picture while the window still draws its pieces and the picture is not shown, so that no
/// frame lacks them.
struct Band {
    tree: Entity,
    anchor: Entity,
    pieces: HashSet<Entity>,
    parts: Parts,
    fresh: bool,
}

/// The bands by their stacking context and their place among its bands.
#[derive(Resource, Default)]
struct Layers(HashMap<(Entity, usize), Band>);

/// The pieces that the bands draw, which the window leaves out ([`hide`]).
#[derive(Resource, Default)]
struct Hidden(HashSet<Entity>);

/// The nodes of the interface, not the layers' own.
type Own = (With<Node>, Without<Composite>, Without<Mirror>);

/// The roots of the window's interface: nodes of no parent that no camera of their own draws.
type Roots = (With<Node>, Without<ChildOf>, Without<UiTargetCamera>);

/// What a band's picture's node reads of the node it lies in, and of the piece it lies before.
type HostData = (
    &'static ComputedNode,
    &'static UiGlobalTransform,
    Option<&'static Children>,
    Option<&'static ChildOf>,
    &'static ZIndex,
);

/// The window's interface's tree, as the layers read it.
#[derive(SystemParam)]
struct Tree<'w, 's> {
    children: Query<'w, 's, &'static Children>,
    nodes: Query<'w, 's, &'static ZIndex, Own>,
    raised: Query<'w, 's, (), With<GlobalZIndex>>,
}

impl Tree<'_, '_> {
    /// The nodes of the stacking context of `context` in the order the interface stacks them:
    /// each node before its children, the children by their `ZIndex`, in their order where it
    /// is the same (`bevy_ui`'s `ui_stack_system`); the subtrees raised by a `GlobalZIndex` of
    /// their own go into `raised`, contexts of their own.
    fn stacked(&self, context: Entity, nodes: &mut Vec<Entity>, raised: &mut Vec<Entity>) {
        nodes.push(context);
        let Ok(children) = self.children.get(context) else {
            return;
        };
        let mut children: Vec<(i32, Entity)> = children
            .iter()
            .filter_map(|child| self.nodes.get(child).ok().map(|z| (z.0, child)))
            .collect();
        children.sort_by_key(|&(z, _)| z);
        for (_, child) in children {
            if self.raised.contains(child) {
                raised.push(child);
            } else {
                self.stacked(child, nodes, raised);
            }
        }
    }

    /// The nodes of the tree of `root`, each before its children, in their order.
    fn all(&self, root: Entity) -> Vec<Entity> {
        let mut found = Vec::new();
        let mut stack = vec![root];
        while let Some(entity) = stack.pop() {
            if !self.nodes.contains(entity) {
                continue;
            }
            found.push(entity);
            if let Ok(children) = self.children.get(entity) {
                stack.extend(children.iter().rev());
            }
        }
        found
    }
}

/// What a node of the interface draws, as the layers see it.
type LookData = (
    &'static Node,
    &'static ComputedNode,
    &'static UiGlobalTransform,
    &'static InheritedVisibility,
    Option<&'static ImageNode>,
    &'static BackgroundColor,
    &'static BorderColor,
    Has<Native>,
    (
        Has<BoxShadow>,
        Has<BackgroundGradient>,
        Has<BorderGradient>,
        Has<Outline>,
    ),
);

/// A node's [`LookData`] as a query gives it.
type Looked<'a> = (
    &'a Node,
    &'a ComputedNode,
    &'a UiGlobalTransform,
    &'a InheritedVisibility,
    Option<&'a ImageNode>,
    &'a BackgroundColor,
    &'a BorderColor,
    bool,
    (bool, bool, bool, bool),
);

/// What a node draws: pieces for a band, something at the window's pixels, or neither.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Look {
    Piece,
    Native,
    Neither,
}

/// Whether a band would lose what the window's pixels show of an image: turned or scaled with its
/// node or one about it (`UiTransform`: the compass's needle, the day's disc), or drawn smaller
/// than its part of its texture, either way resampled at the original's pixels. An image
/// stretched over a node larger than its part (a button's middle) loses nothing: a band
/// magnifies it as the window would, and keeps its joins.
fn sharper_apart(
    image: &ImageNode,
    (computed, transform): (&ComputedNode, &UiGlobalTransform),
    textures: &Assets<Image>,
) -> bool {
    if !transform.matrix2.abs_diff_eq(Mat2::IDENTITY, 1e-4) {
        return true;
    }
    let texels = match image.rect {
        Some(rect) => rect.size(),
        None => match textures.get(&image.image) {
            Some(texture) => texture.size_f32(),
            None => return false,
        },
    };
    let size = computed.unrounded_size * computed.inverse_scale_factor();
    (texels - size).max_element() > 0.5
}

/// What a node of the interface draws: a piece, an image or a background shown; something at the
/// window's pixels, a material's, a camera's picture (of `pictures`), an image
/// [`sharper_apart`], a shadow, a gradient, an outline or a border, whatever else it draws;
/// neither, a text's glyphs alone, a node hidden or of nothing shown.
fn look(
    (node, computed, transform, shown, image, background, border, native, others): Looked<'_>,
    (pictures, textures): (&HashSet<AssetId<Image>>, &Assets<Image>),
) -> Look {
    if !shown.get() {
        return Look::Neither;
    }
    let (shadow, gradient, border_gradient, outline) = others;
    let bordered = node.border != UiRect::ZERO
        && [border.top, border.right, border.bottom, border.left]
            .iter()
            .any(|colour| colour.alpha() > 0.0);
    let image = image.filter(|image| image.color.alpha() > 0.0);
    let apart = image.is_some_and(|image| {
        pictures.contains(&image.image.id())
            || sharper_apart(image, (computed, transform), textures)
    });
    if native || shadow || gradient || border_gradient || outline || bordered || apart {
        return Look::Native;
    }
    if image.is_some() || background.0.alpha() > 0.0 {
        Look::Piece
    } else {
        Look::Neither
    }
}

/// The pictures that cameras draw into, which an image draws at the window's pixels: the items'
/// models, the menus' water.
fn camera_pictures<'a>(targets: impl Iterator<Item = &'a RenderTarget>) -> HashSet<AssetId<Image>> {
    targets
        .filter_map(|target| match target {
            RenderTarget::Image(target) => Some(target.handle.id()),
            _ => None,
        })
        .collect()
}

/// The bands of a stacking context's nodes in the order the interface stacks them: the runs of
/// pieces between the nodes that draw at the window's pixels, of [`LEAST_PIECES`] or more.
fn cut(looks: impl IntoIterator<Item = (Entity, Look)>) -> Vec<Vec<Entity>> {
    let mut bands = Vec::new();
    let mut band = Vec::new();
    for (entity, look) in looks {
        match look {
            Look::Piece => band.push(entity),
            Look::Native => {
                let run = std::mem::take(&mut band);
                if run.len() >= LEAST_PIECES {
                    bands.push(run);
                }
            }
            Look::Neither => {}
        }
    }
    if band.len() >= LEAST_PIECES {
        bands.push(band);
    }
    bands
}

/// The window's size in the original's pixels, and the interface's scale: the frames' size, and
/// the layers' cameras' scale.
fn window_size(window: &Window, scale: f32) -> (Vec2, f32) {
    let scale = scale.max(0.01);
    (Vec2::new(window.width(), window.height()) / scale, scale)
}

/// A band's picture's size for the window's `whole` size in the original's pixels: whole
/// pixels, a texel to an original's pixel.
fn picture_size(whole: Vec2) -> UVec2 {
    whole.ceil().as_uvec2().max(UVec2::ONE)
}

/// A frame of `whole` size in the original's pixels, laying its root out as the window's
/// viewport lays out a root of its own (`bevy_ui`'s implicit viewport node).
fn frame_style(whole: Vec2) -> Node {
    Node {
        display: Display::Grid,
        width: Val::Px(whole.x),
        height: Val::Px(whole.y),
        align_items: AlignItems::Start,
        justify_items: JustifyItems::Start,
        ..default()
    }
}

/// A band's picture of `size`: its camera's last pass encodes the frame's values into it, and
/// `layer.wesl` reads them back texel by texel.
fn picture_image(size: UVec2) -> Image {
    Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None)
}

/// The node that draws a band's picture of `material` in the window, at `place`.
fn composite_bundle(material: &Handle<Picture>, place: Node, z: ZIndex) -> impl Bundle {
    (
        Composite,
        MaterialNode(material.clone()),
        place,
        z,
        Pickable::IGNORE,
    )
}

/// A band's parts for the tree of `tree`, at the window's `whole` size in the original's pixels
/// and the interface's `scale`.
fn make(
    commands: &mut Commands,
    (pictures, materials): (&mut Assets<Image>, &mut Assets<Picture>),
    tree: Entity,
    (whole, scale): (Vec2, f32),
) -> Parts {
    let picture = pictures.add(picture_image(picture_size(whole)));
    let material = materials.add(Picture {
        picture: picture.clone(),
    });
    let camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: ORDER,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(ImageRenderTarget {
                handle: picture.clone(),
                scale_factor: 1.0 / scale,
            }),
            CompositingSpace::Srgb,
            RenderLayers::layer(LAYER),
            Msaa::Off,
            Tonemapping::None,
            DebandDither::Disabled,
            // A picture of its own, out of the queries of the world's camera (`iso::Eye`), which
            // take one, and of its zoom (#2639).
            crate::iso::Apart,
        ))
        .id();
    let frame = commands
        .spawn((frame_style(whole), UiTargetCamera(camera), Pickable::IGNORE))
        .id();
    let root = commands
        .spawn((Node::default(), Mirror, Pickable::IGNORE, ChildOf(frame)))
        .id();
    let composite = commands
        .spawn(composite_bundle(
            &material,
            Node::default(),
            ZIndex::default(),
        ))
        .id();
    Parts {
        camera,
        picture,
        material,
        frame,
        composite,
        whole,
        mirrors: HashMap::from([(tree, root)]),
    }
}

/// Despawns a band's parts: its camera, its frame with its mirror, its picture's node.
fn end(commands: &mut Commands, parts: &Parts) {
    commands.entity(parts.camera).try_despawn();
    commands.entity(parts.frame).try_despawn();
    commands.entity(parts.composite).try_despawn();
}

/// Cuts the shown interface's stacking contexts into their bands, makes the parts of the new and
/// ends those of the gone, keeps the pictures at the window's size, and hands the window's
/// drawing back the pieces that no band draws any longer.
#[expect(clippy::too_many_arguments, reason = "a Bevy system's parameters")]
fn bands(
    mut commands: Commands,
    (mut layers, mut hidden): (ResMut<Layers>, ResMut<Hidden>),
    (window, scale): (Query<&Window, With<PrimaryWindow>>, Res<UiScale>),
    roots: Query<(Entity, &InheritedVisibility), Roots>,
    tree: Tree,
    looks: Query<LookData, Own>,
    mut cameras: Query<&mut RenderTarget, With<Camera>>,
    (mut pictures, mut materials): (ResMut<Assets<Image>>, ResMut<Assets<Picture>>),
) {
    let Ok(window) = window.single() else {
        return;
    };
    let (whole, scale) = window_size(window, scale.0);
    let native = camera_pictures(cameras.iter());
    let mut found = Vec::new();
    for (root, shown) in &roots {
        if !shown.get() || !tree.nodes.contains(root) {
            continue;
        }
        let mut contexts = vec![root];
        while let Some(context) = contexts.pop() {
            let mut stacked = Vec::new();
            tree.stacked(context, &mut stacked, &mut contexts);
            let looks = stacked.into_iter().map(|entity| {
                let look = looks
                    .get(entity)
                    .map_or(Look::Neither, |data| look(data, (&native, &pictures)));
                (entity, look)
            });
            for (index, pieces) in cut(looks).into_iter().enumerate() {
                found.push(((context, index), root, pieces));
            }
        }
    }
    let wanted: HashSet<(Entity, usize)> = found.iter().map(|(key, ..)| *key).collect();
    layers.0.retain(|key, band| {
        let kept = wanted.contains(key);
        if !kept {
            end(&mut commands, &band.parts);
        }
        kept
    });
    let mut drawn = HashSet::new();
    for (key, root, pieces) in found {
        let Some(&anchor) = pieces.first() else {
            continue;
        };
        let fresh = !layers.0.contains_key(&key);
        let band = layers.0.entry(key).or_insert_with(|| Band {
            tree: root,
            anchor,
            pieces: HashSet::new(),
            parts: make(
                &mut commands,
                (&mut pictures, &mut materials),
                root,
                (whole, scale),
            ),
            fresh,
        });
        band.fresh = fresh;
        if band.tree != root {
            end(&mut commands, &band.parts);
            band.parts = make(
                &mut commands,
                (&mut pictures, &mut materials),
                root,
                (whole, scale),
            );
            band.tree = root;
            band.fresh = true;
        }
        band.anchor = anchor;
        band.pieces = pieces.into_iter().collect();
        if !band.fresh {
            drawn.extend(band.pieces.iter().copied());
        }
        let parts = &mut band.parts;
        if parts.whole != whole {
            parts.whole = whole;
            commands.entity(parts.frame).insert(frame_style(whole));
            let size = picture_size(whole);
            if let Err(error) = pictures.insert(&parts.picture, picture_image(size)) {
                warn!("ui layers: the picture at {size}: {error}");
            }
            // The material's bind group made again with the picture's new texture: an asset is
            // marked changed only as it is written.
            if let Some(mut material) = materials.get_mut(&parts.material) {
                material.picture = parts.picture.clone();
            }
            if let Ok(mut target) = cameras.get_mut(parts.camera) {
                *target = RenderTarget::Image(ImageRenderTarget {
                    handle: parts.picture.clone(),
                    scale_factor: 1.0 / scale,
                });
            }
        }
    }
    // The pieces no band draws any longer, extracted again for the window to draw.
    for &entity in hidden.0.difference(&drawn) {
        if let Ok((_, _, _, _, _, background, ..)) = looks.get(entity) {
            commands.entity(entity).try_insert(*background);
        }
    }
    hidden.0 = drawn;
}

/// The mirror's style of an original: its own, a text's at the size its text took.
fn mirrored_style(node: &Node, text: bool, computed: &ComputedNode) -> Node {
    let mut style = node.clone();
    if text {
        let size = computed.unrounded_size * computed.inverse_scale_factor();
        style.width = Val::Px(size.x);
        style.height = Val::Px(size.y);
        style.min_width = Val::Auto;
        style.min_height = Val::Auto;
        style.max_width = Val::Auto;
        style.max_height = Val::Auto;
    }
    style
}

/// What a mirror copies of its original.
type OriginalData = (
    &'static Node,
    &'static ComputedNode,
    Has<Text>,
    Option<Ref<'static, ImageNode>>,
    &'static BackgroundColor,
    &'static Visibility,
    &'static ZIndex,
    &'static UiTransform,
    &'static ScrollPosition,
);

/// What a mirror keeps of its original.
type MirrorData = (
    &'static mut Node,
    Option<&'static mut ImageNode>,
    &'static mut BackgroundColor,
    Option<&'static Children>,
    &'static mut Visibility,
    &'static mut ZIndex,
    &'static mut UiTransform,
    &'static mut ScrollPosition,
);

/// Keeps `value` at `wanted`, marking it changed only when it is not.
fn keep<T: PartialEq>(mut value: Mut<T>, wanted: T) {
    if *value != wanted {
        *value = wanted;
    }
}

/// Keeps each band's mirror of its root's tree: a node for each of the tree's, spawned for the
/// new and despawned for the gone, their styles, orders, visibility, turns, scrolls and images
/// the originals', and the colours of the band's pieces alone.
fn mirror(
    mut commands: Commands,
    mut layers: ResMut<Layers>,
    tree: Tree,
    originals: Query<OriginalData, Without<Mirror>>,
    mut mirrors: Query<MirrorData, With<Mirror>>,
) {
    for band in layers.0.values_mut() {
        let nodes = tree.all(band.tree);
        let present: HashSet<Entity> = nodes.iter().copied().collect();
        band.parts.mirrors.retain(|original, mirror| {
            let kept = present.contains(original);
            if !kept {
                commands.entity(*mirror).try_despawn();
            }
            kept
        });
        for &original in &nodes {
            let Ok((style, computed, text, image, background, visibility, z, turn, scroll)) =
                originals.get(original)
            else {
                continue;
            };
            let piece = band.pieces.contains(&original);
            let colour = |colour: Color| if piece { colour } else { Color::NONE };
            let mirror = *band.parts.mirrors.entry(original).or_insert_with(|| {
                commands
                    .spawn((Node::default(), Mirror, Pickable::IGNORE))
                    .id()
            });
            let style = mirrored_style(style, text, computed);
            let copy = |image: &ImageNode| ImageNode {
                color: colour(image.color),
                ..image.clone()
            };
            let Ok((node, mirror_image, mirror_background, _, shown, order, turned, mut scrolled)) =
                mirrors.get_mut(mirror)
            else {
                let mut entity = commands.entity(mirror);
                entity.insert((
                    style,
                    BackgroundColor(colour(background.0)),
                    *visibility,
                    *z,
                    *turn,
                    ScrollPosition(scroll.0),
                ));
                if let Some(image) = image {
                    entity.insert(copy(&image));
                }
                continue;
            };
            keep(node, style);
            keep(mirror_background, BackgroundColor(colour(background.0)));
            keep(shown, *visibility);
            keep(order, *z);
            keep(turned, *turn);
            if scrolled.0 != scroll.0 {
                scrolled.0 = scroll.0;
            }
            match (image, mirror_image) {
                (Some(image), Some(mut mirrored)) => {
                    if image.is_changed() {
                        *mirrored = copy(&image);
                    } else {
                        let wanted = colour(image.color);
                        if mirrored.color != wanted {
                            mirrored.color = wanted;
                        }
                    }
                }
                (Some(image), None) => {
                    commands.entity(mirror).insert(copy(&image));
                }
                (None, Some(_)) => {
                    commands.entity(mirror).remove::<ImageNode>();
                }
                (None, None) => {}
            }
        }
        // Each mirror's children in its original's order; the tree's root's mirror stays in
        // its frame.
        for &original in &nodes {
            let Some(&mirror) = band.parts.mirrors.get(&original) else {
                continue;
            };
            let wanted: Vec<Entity> = tree
                .children
                .get(original)
                .map(|children| {
                    children
                        .iter()
                        .filter(|child| present.contains(child))
                        .filter_map(|child| band.parts.mirrors.get(&child).copied())
                        .collect()
                })
                .unwrap_or_default();
            let current: Vec<Entity> = mirrors
                .get(mirror)
                .ok()
                .and_then(|(_, _, _, children, ..)| children)
                .map(|children| children.iter().collect())
                .unwrap_or_default();
            if current != wanted {
                commands.entity(mirror).replace_children(&wanted);
            }
        }
    }
}

/// Keeps each band's picture's node where its first piece stacks: the first child of a context's
/// root that is its first piece, else just before the first piece among its parent's children
/// at its `ZIndex`; over the whole window, placed by its parent's layout of the frame before
/// ([`centre`] puts it right as the layout ends).
fn composites(
    mut commands: Commands,
    mut layers: ResMut<Layers>,
    hosts: Query<HostData, Without<Composite>>,
    mut placed: Query<(&mut Node, &mut ZIndex, &mut Visibility), With<Composite>>,
) {
    for (&(context, _), band) in &mut layers.0 {
        let anchor = band.anchor;
        let (host, z, before) = if anchor == context {
            (anchor, ZIndex(i32::MIN), None)
        } else {
            let Ok((.., Some(parent), &z)) = hosts.get(anchor) else {
                continue;
            };
            (parent.parent(), z, Some(anchor))
        };
        let Ok((computed, transform, children, ..)) = hosts.get(host) else {
            continue;
        };
        // The top left of the host's padding box, which its absolute children are placed in, in
        // the original's pixels: the picture's place relative to it.
        let corner =
            (transform.translation + computed.padding_box().min) * computed.inverse_scale_factor();
        let size = picture_size(band.parts.whole).as_vec2();
        let place = Node {
            position_type: PositionType::Absolute,
            left: Val::Px(-corner.x),
            top: Val::Px(-corner.y),
            width: Val::Px(size.x),
            height: Val::Px(size.y),
            ..default()
        };
        let composite = band.parts.composite;
        let shown = if band.fresh {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if let Ok((node, order, visibility)) = placed.get_mut(composite) {
            keep(node, place);
            keep(order, z);
            keep(visibility, shown);
        } else {
            // Gone with its host's children, made again.
            band.parts.composite = commands
                .spawn(composite_bundle(&band.parts.material, place, z))
                .id();
        }
        let composite = band.parts.composite;
        let others: Vec<Entity> = children
            .map(|children| {
                children
                    .iter()
                    .filter(|&child| child != composite)
                    .collect()
            })
            .unwrap_or_default();
        let index = before
            .and_then(|anchor| others.iter().position(|&child| child == anchor))
            .unwrap_or(0);
        let now =
            children.and_then(|children| children.iter().position(|child| child == composite));
        if now != Some(index) {
            commands.entity(host).insert_children(index, &[composite]);
        }
    }
}

/// Puts each band's picture over the window's top left as the layout ends: [`composites`] places
/// it by its host's layout of the frame before, which a host shown or moved this frame left
/// behind.
fn centre(mut composites: Query<(&ComputedNode, &mut UiGlobalTransform), With<Composite>>) {
    for (computed, transform) in &mut composites {
        keep(
            transform,
            UiGlobalTransform::from_translation(computed.size() * 0.5),
        );
    }
}

/// Leaves out of the window's drawing the images and the backgrounds of the pieces that the
/// bands draw, as Bevy extracts the interface: its extraction keeps a node's parts until the node
/// changes, so a piece's are left out once, and again whenever it changes.
fn hide(
    hidden: Extract<Res<Hidden>>,
    mut extracted: ResMut<ExtractedUiNodes>,
    mut commands: Commands,
) {
    for &entity in &hidden.0 {
        let Some((_, items)) = extracted.uinodes.get_mut(&MainEntity::from(entity)) else {
            continue;
        };
        items.retain(|&item, node| {
            let piece = matches!(
                node.item,
                ExtractedUiItem::Node {
                    node_type: NodeType::Rect,
                    ..
                }
            );
            if piece {
                commands.entity(item).try_despawn();
            }
            !piece
        });
    }
}

/// The interface's layers.
pub struct UiLayersPlugin;

impl Plugin for UiLayersPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "layer.wesl");
        app.add_plugins(UiMaterialPlugin::<Picture>::default())
            .init_resource::<Layers>()
            .init_resource::<Hidden>()
            .add_systems(
                PostUpdate,
                (bands, mirror, composites)
                    .chain()
                    .before(CameraUpdateSystems)
                    .before(UiSystems::Prepare),
            )
            .add_systems(PostUpdate, centre.in_set(UiSystems::PostLayout));
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                ExtractSchedule,
                hide.after(RenderUiSystems::ExtractBackgrounds)
                    .after(RenderUiSystems::ExtractImages)
                    .after(RenderUiSystems::ExtractTextBackgrounds),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).unwrap_or(Entity::PLACEHOLDER)
    }

    #[test]
    fn bands_are_cut_at_what_draws_at_the_windows_pixels() {
        let looks = [
            (entity(1), Look::Native),
            (entity(2), Look::Piece),
            (entity(3), Look::Neither),
            (entity(4), Look::Piece),
            (entity(5), Look::Native),
            (entity(6), Look::Piece),
            (entity(7), Look::Native),
            (entity(8), Look::Piece),
            (entity(9), Look::Piece),
            (entity(10), Look::Piece),
        ];
        assert_eq!(
            cut(looks),
            vec![
                vec![entity(2), entity(4)],
                vec![entity(8), entity(9), entity(10)]
            ]
        );
    }

    #[test]
    fn a_picture_holds_the_window_in_whole_original_pixels() {
        assert_eq!(
            picture_size(Vec2::new(1245.45, 768.0)),
            UVec2::new(1246, 768)
        );
        assert_eq!(picture_size(Vec2::ZERO), UVec2::ONE);
    }

    #[test]
    fn an_image_turned_scaled_or_drawn_smaller_is_drawn_apart() {
        let textures = Assets::<Image>::default();
        let node = |size: Vec2, inverse_scale_factor: f32| ComputedNode {
            unrounded_size: size,
            inverse_scale_factor,
            ..ComputedNode::DEFAULT
        };
        let part = ImageNode {
            rect: Some(Rect::new(0.0, 0.0, 32.0, 16.0)),
            ..default()
        };
        let still = UiGlobalTransform::default();
        let apart = |computed: &ComputedNode, transform: &UiGlobalTransform| {
            sharper_apart(&part, (computed, transform), &textures)
        };
        // One to one, at the window's scale 2 too, and stretched over a larger node: assembled.
        assert!(!apart(&node(Vec2::new(32.0, 16.0), 1.0), &still));
        assert!(!apart(&node(Vec2::new(64.0, 32.0), 0.5), &still));
        assert!(!apart(&node(Vec2::new(200.0, 16.0), 1.0), &still));
        // Drawn smaller than its part, turned or scaled: apart.
        assert!(apart(&node(Vec2::new(16.0, 16.0), 1.0), &still));
        let turned = UiGlobalTransform::from_rotation(Rot2::degrees(30.0));
        assert!(apart(&node(Vec2::new(32.0, 16.0), 1.0), &turned));
        let scaled = UiGlobalTransform::from_scale(Vec2::new(2.0, 1.0));
        assert!(apart(&node(Vec2::new(32.0, 16.0), 1.0), &scaled));
    }

    /// The world's camera stays the one camera of `iso::Eye` with a band's beside it (#2639).
    #[test]
    fn a_bands_camera_is_none_of_the_worlds() {
        let mut world = World::new();
        let world_camera = world.spawn(Camera3d::default()).id();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut pictures = Assets::<Image>::default();
        let mut materials = Assets::<Picture>::default();
        let tree = world.spawn_empty().id();
        let parts = {
            let mut commands = Commands::new(&mut queue, &world);
            make(
                &mut commands,
                (&mut pictures, &mut materials),
                tree,
                (Vec2::new(1024.0, 768.0), 1.0),
            )
        };
        queue.apply(&mut world);
        assert!(world.get::<Camera>(parts.camera).is_some());
        let mut eyes = world.query_filtered::<Entity, crate::iso::Eye<With<Camera>>>();
        assert_eq!(eyes.iter(&world).collect::<Vec<_>>(), vec![world_camera]);
    }
}
