//! The items' pictures drawn only when they change (#2660): the cameras that draw the items'
//! models into pictures of the interface (the weapon sets' on the bar, the backpack's, the
//! equipment's, the merchant's, the smith's, the stashes', the tooltips' sockets', the combat
//! master's runes, the held item's) draw in the frames after something of their render layer
//! changed, and their pictures keep their last drawing between, as a camera made inactive keeps
//! its target's. Research #2637 measured 1.89 ms a frame saved at 3840 × 2160 with the
//! inventory open.
//!
//! A change is a part of the layer made, gone, moved, shown or hidden, or given another mesh,
//! material, layer or light; a material of its parts changed (an effect's offsets, which run on
//! the clock while the item shows, `model_effects`); a mesh or a texture of its parts still
//! loading; or the camera's own place, projection, target or light. The camera draws [`FRAMES`]
//! frames after each, which covers a scene's parts coming in over frames and a texture's way to
//! the GPU, and on while any render pipeline is still being made, as a pipeline being compiled
//! draws nothing (`ui_layers::Settled`, #2640).
//!
//! A picture is drawn at the window's pixels, and its target follows the window's size
//! ([`WindowPixels`], #2697): made again at the window's scale as the window's size changes, as
//! the interface's layers' pictures are (`ui_layers`). Made once, the picture of the bar's weapon
//! sets kept the pixels of the window that a game began in, and F11's full screen drew it
//! stretched.

use bevy::asset::{AssetEvent, AssetEventSystems, AssetId};
use bevy::camera::visibility::{RenderLayers, VisibilitySystems};
use bevy::camera::{CameraUpdateSystems, RenderTarget, ScalingMode};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::ecs::query::QueryItem;
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat};
use bevy::transform::TransformSystems;
use bevy::ui_render::prelude::MaterialNode;
use bevy::window::PrimaryWindow;

use crate::bar::SCREEN;
use crate::choice::Layer;
use crate::game::AppState;
use crate::iso::Apart;
use crate::model_effects::ApartMaterial;
use crate::ui_layers::Settled;

/// The frames a picture's camera draws after a change: the change's own, and three more, as the
/// render world runs a frame behind the main world and a material may take a texture that came in
/// only a frame after it.
const FRAMES: u8 = 4;

/// A camera of an item's picture that draws only after a change of its render layer: the frames
/// it has still to draw, and the parts of its layer it saw in the last frame.
#[derive(Component, Default)]
pub(crate) struct DrawnOnChange {
    left: u8,
    parts: Vec<Entity>,
}

/// What tells of a change of a part of a picture's layer.
type PartData = (
    Entity,
    Ref<'static, RenderLayers>,
    Option<Ref<'static, GlobalTransform>>,
    (
        Option<Ref<'static, Visibility>>,
        Option<Ref<'static, InheritedVisibility>>,
    ),
    Option<Ref<'static, Mesh3d>>,
    Option<Ref<'static, MeshMaterial3d<StandardMaterial>>>,
    Option<Ref<'static, MeshMaterial3d<ApartMaterial>>>,
    Option<Ref<'static, DirectionalLight>>,
);

/// What tells of a change of a picture's camera itself.
type CameraData = (
    &'static mut Camera,
    &'static mut DrawnOnChange,
    &'static RenderLayers,
    Ref<'static, GlobalTransform>,
    Ref<'static, Projection>,
    Ref<'static, RenderTarget>,
    Option<Ref<'static, AmbientLight>>,
);

/// The ids of the assets of the kind `A` made, changed or loaded since the last frame.
fn changed<A: Asset>(events: &mut MessageReader<AssetEvent<A>>) -> HashSet<AssetId<A>> {
    events
        .read()
        .filter_map(|event| match event {
            AssetEvent::Added { id }
            | AssetEvent::Modified { id }
            | AssetEvent::LoadedWithDependencies { id } => Some(*id),
            AssetEvent::Removed { .. } | AssetEvent::Unused { .. } => None,
        })
        .collect()
}

/// Whether a texture of `material` is still loading.
fn loading(material: &StandardMaterial, server: &AssetServer) -> bool {
    [
        &material.base_color_texture,
        &material.emissive_texture,
        &material.metallic_roughness_texture,
        &material.normal_map_texture,
        &material.occlusion_texture,
    ]
    .into_iter()
    .flatten()
    .any(|texture| server.load_state(texture).is_loading())
}

/// The materials of the parts' meshes, and the assets that changed since the last frame.
struct Looks<'a> {
    standard: &'a Assets<StandardMaterial>,
    apart: &'a Assets<ApartMaterial>,
    server: &'a AssetServer,
    changed_standard: HashSet<AssetId<StandardMaterial>>,
    changed_apart: HashSet<AssetId<ApartMaterial>>,
}

/// Whether the part `part` changed since the last frame, or still waits for its mesh or a
/// texture.
fn part_changed(
    (_, layers, place, (shown, inherited), mesh, standard, apart, light): &QueryItem<
        '_,
        '_,
        PartData,
    >,
    assets: &Looks,
) -> bool {
    let component = layers.is_changed()
        || place.as_ref().is_some_and(DetectChanges::is_changed)
        || shown.as_ref().is_some_and(DetectChanges::is_changed)
        || inherited.as_ref().is_some_and(DetectChanges::is_changed)
        || mesh.as_ref().is_some_and(DetectChanges::is_changed)
        || standard.as_ref().is_some_and(DetectChanges::is_changed)
        || apart.as_ref().is_some_and(DetectChanges::is_changed)
        || light.as_ref().is_some_and(DetectChanges::is_changed);
    let mesh_loading = mesh
        .as_ref()
        .is_some_and(|mesh| assets.server.load_state(mesh.id()).is_loading());
    let standard_changed = standard.as_ref().is_some_and(|material| {
        assets.changed_standard.contains(&material.id())
            || assets
                .standard
                .get(material.id())
                .is_some_and(|material| loading(material, assets.server))
    });
    let apart_changed = apart.as_ref().is_some_and(|material| {
        assets.changed_apart.contains(&material.id())
            || assets
                .apart
                .get(material.id())
                .is_some_and(|material| loading(&material.base, assets.server))
    });
    component || mesh_loading || standard_changed || apart_changed
}

/// Counts down the frames a picture's camera has still to draw (`left`), [`FRAMES`] again after
/// a change (`marked`) and while a pipeline is still being made (`settled` not): whether it
/// draws this frame.
fn drawing(left: &mut u8, marked: bool, settled: bool) -> bool {
    if marked || !settled {
        *left = FRAMES;
    }
    let drawing = *left > 0;
    *left = left.saturating_sub(1);
    drawing
}

/// The assets' events that tell of a change of a part's material or of a picture's target.
type AssetChanges<'w, 's> = (
    MessageReader<'w, 's, AssetEvent<StandardMaterial>>,
    MessageReader<'w, 's, AssetEvent<ApartMaterial>>,
    MessageReader<'w, 's, AssetEvent<Image>>,
);

/// Lets each picture's camera draw only in the frames after a change of its render layer or of
/// itself, and while a pipeline is still being made.
fn draw_on_change(
    mut cameras: Query<CameraData>,
    parts: Query<PartData, Without<Camera>>,
    (standard, apart, server): (
        Res<Assets<StandardMaterial>>,
        Res<Assets<ApartMaterial>>,
        Res<AssetServer>,
    ),
    (mut standard_events, mut apart_events, mut image_events): AssetChanges,
    settled: Option<Res<Settled>>,
) {
    let assets = Looks {
        standard: &standard,
        apart: &apart,
        server: &server,
        changed_standard: changed(&mut standard_events),
        changed_apart: changed(&mut apart_events),
    };
    let images = changed(&mut image_events);
    let settled = settled.is_none_or(|settled| settled.made());
    if cameras.is_empty() {
        return;
    }
    let watched = cameras
        .iter()
        .fold(RenderLayers::none(), |all, (_, _, layers, ..)| {
            all.union(layers)
        });
    let parts: Vec<_> = parts
        .iter()
        .filter(|(_, layers, ..)| layers.intersects(&watched))
        .collect();
    for (mut camera, mut on, layers, place, projection, target, light) in &mut cameras {
        let mut marked = on.is_added()
            || place.is_changed()
            || projection.is_changed()
            || target.is_changed()
            || light.is_some_and(|light| light.is_changed())
            || target
                .as_image()
                .is_some_and(|image| images.contains(&image.id()));
        let mut seen = Vec::new();
        for part in parts.iter().filter(|(_, own, ..)| own.intersects(layers)) {
            seen.push(part.0);
            marked = marked || part_changed(part, &assets);
        }
        seen.sort_unstable();
        if seen != on.parts {
            on.parts = seen;
            marked = true;
        }
        let draws = drawing(&mut on.left, marked, settled);
        if camera.is_active != draws {
            camera.is_active = draws;
        }
    }
}

/// What a picture's camera draws, in the original's pixels: a part of the original's screen of
/// its size, as the interface scales it, or the whole window (the held item's, which follows the
/// cursor anywhere).
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub(crate) enum WindowPixels {
    Part(Vec2),
    Whole,
}

impl WindowPixels {
    /// The picture's size in the original's pixels in a window of `window` pixels.
    fn size(self, window: Vec2) -> Vec2 {
        match self {
            Self::Part(size) => size,
            Self::Whole => window / scale(window),
        }
    }
}

/// The interface's scale in a window of `window` pixels: the original's 768 rows to its height.
fn scale(window: Vec2) -> f32 {
    (window.y / SCREEN.y).max(0.01)
}

/// The window's pixels of a picture of `size` in the original's pixels, at the interface's
/// `scale`.
pub(crate) fn pixels(size: Vec2, scale: f32) -> UVec2 {
    (size * scale).round().as_uvec2().max(UVec2::ONE)
}

/// A picture's target of `pixels`.
pub(crate) fn target(pixels: UVec2) -> Image {
    Image::new_target_texture(pixels.x, pixels.y, TextureFormat::Rgba8UnormSrgb, None)
}

/// A picture's camera's projection and place for its `size` in the original's pixels: a unit of
/// the models a pixel of the original's screen from the picture's top left, the rows going down.
fn view(size: Vec2) -> (Projection, Transform) {
    (
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: size.x,
                height: size.y,
            },
            near: -2_000.0,
            far: 2_000.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(size.x / 2.0, -size.y / 2.0, 0.0),
    )
}

/// A picture's camera of the order `order` that draws the render layer `layer` into `target`, of
/// `picture` (`size` in the original's pixels): lit by an ambient light and the directional
/// light of its layer alone, drawn only after a change ([`DrawnOnChange`]), its target following
/// the window's size ([`WindowPixels`]).
pub(crate) fn camera(
    target: Handle<Image>,
    (picture, size): (WindowPixels, Vec2),
    (order, layer): (isize, usize),
) -> impl Bundle {
    let (projection, place) = view(size);
    (
        (
            Camera3d::default(),
            Camera {
                order,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::from(target),
            projection,
            place,
            Tonemapping::None,
            DebandDither::Disabled,
        ),
        // Lit by an ambient and a directional light alone: one cluster for the lights (#2664).
        bevy::light::cluster::ClusterConfig::Single,
        AmbientLight {
            color: Color::WHITE,
            brightness: crate::backpack::AMBIENT,
            affects_lightmapped_meshes: true,
        },
        RenderLayers::layer(layer),
        Apart,
        DrawnOnChange::default(),
        picture,
        DespawnOnExit(AppState::InGame),
    )
}

/// Keeps each picture's target at the window's pixels as the window's size changes: the target
/// made again at the window's scale, the camera's projection over the picture's size, and the
/// material that shows the target written, as an asset is marked changed only as it is written
/// and its bind group holds the old texture till then (`ui_layers::bands`). A minimised window,
/// of no rows, leaves them as they are.
fn follow_window(
    window: Option<Single<&Window, With<PrimaryWindow>>>,
    mut cameras: Query<(
        &WindowPixels,
        &RenderTarget,
        &mut Projection,
        &mut Transform,
    )>,
    (mut images, mut layers): (ResMut<Assets<Image>>, ResMut<Assets<Layer>>),
    nodes: Query<&MaterialNode<Layer>>,
) {
    let Some(window) = window else {
        return;
    };
    let whole = Vec2::new(window.width(), window.height());
    if whole.y < 1.0 {
        return;
    }
    for (picture, target, mut projection, mut place) in &mut cameras {
        let Some(handle) = target.as_image() else {
            continue;
        };
        let size = picture.size(whole);
        let wanted = pixels(size, scale(whole));
        if images
            .get(handle)
            .is_none_or(|image| image.size() == wanted)
        {
            continue;
        }
        if let Some(mut image) = images.get_mut(handle) {
            image.resize(Extent3d {
                width: wanted.x,
                height: wanted.y,
                depth_or_array_layers: 1,
            });
        }
        (*projection, *place) = view(size);
        for node in &nodes {
            if layers
                .get(&node.0)
                .is_some_and(|layer| layer.figures == *handle)
                && let Some(mut layer) = layers.get_mut(&node.0)
            {
                layer.figures = handle.clone();
            }
        }
    }
}

/// The items' pictures drawn only when they change.
pub struct ItemPicturesPlugin;

impl Plugin for ItemPicturesPlugin {
    fn build(&self, app: &mut App) {
        // After the frame's moves, shows and assets' changes are told, before the cameras'
        // visible entities are found, which an inactive camera's are not.
        app.add_systems(
            PostUpdate,
            (
                // Before the cameras' targets are measured, its change told with the frame's
                // assets' changes, and the places propagated.
                follow_window
                    .before(AssetEventSystems)
                    .before(CameraUpdateSystems)
                    .before(TransformSystems::Propagate),
                draw_on_change
                    .after(AssetEventSystems)
                    .after(TransformSystems::Propagate)
                    .after(VisibilitySystems::VisibilityPropagate)
                    .before(VisibilitySystems::CheckVisibility),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use bevy::asset::AssetPlugin;
    use bevy::transform::TransformPlugin;

    use super::*;

    /// A picture's layer and another.
    const LAYER: usize = 10;
    const OTHER: usize = 11;

    fn app(settled: &Settled) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
            .init_asset::<Image>()
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<ApartMaterial>()
            .init_asset::<Layer>()
            .insert_resource(settled.clone())
            .add_plugins(ItemPicturesPlugin);
        app
    }

    /// A picture's camera of the layer [`LAYER`].
    fn camera(app: &mut App) -> Entity {
        app.world_mut()
            .spawn((
                Camera::default(),
                Projection::default(),
                RenderTarget::default(),
                RenderLayers::layer(LAYER),
                DrawnOnChange::default(),
            ))
            .id()
    }

    /// Whether the camera draws, in each of `frames` frames.
    fn draws(app: &mut App, camera: Entity, frames: usize) -> Vec<bool> {
        (0..frames)
            .map(|_| {
                app.update();
                app.world()
                    .get::<Camera>(camera)
                    .is_some_and(|camera| camera.is_active)
            })
            .collect()
    }

    /// Moves the entity's transform along x.
    fn shift(app: &mut App, entity: Entity) {
        if let Some(mut place) = app.world_mut().get_mut::<Transform>(entity) {
            place.translation.x += 5.0;
        }
    }

    #[test]
    fn a_picture_draws_after_its_layer_changes_and_keeps_its_drawing_between() {
        let settled = Settled::default();
        settled.tell(true);
        let mut app = app(&settled);
        let camera = camera(&mut app);
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let part = app
            .world_mut()
            .spawn((
                Transform::default(),
                Visibility::default(),
                MeshMaterial3d(material.clone()),
                RenderLayers::layer(LAYER),
            ))
            .id();
        let other = app
            .world_mut()
            .spawn((Transform::default(), RenderLayers::layer(OTHER)))
            .id();
        // Made: its first frames, then none.
        assert_eq!(
            draws(&mut app, camera, 6),
            [true, true, true, true, false, false]
        );
        let again = [true, true, true, true, false];
        // A part moved.
        shift(&mut app, part);
        assert_eq!(draws(&mut app, camera, 5), again);
        // Another layer's part moved: nothing to draw.
        shift(&mut app, other);
        assert_eq!(draws(&mut app, camera, 2), [false, false]);
        // A part's material changed, as an effect's offsets run on the clock.
        if let Some(mut changed) = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut(&material)
        {
            changed.base_color = Color::BLACK;
        }
        assert_eq!(draws(&mut app, camera, 5), again);
        // A part hidden.
        if let Some(mut shown) = app.world_mut().get_mut::<Visibility>(part) {
            *shown = Visibility::Hidden;
        }
        assert_eq!(draws(&mut app, camera, 5), again);
        // A part gone.
        app.world_mut().despawn(part);
        assert_eq!(draws(&mut app, camera, 5), again);
        // A part made.
        app.world_mut()
            .spawn((Transform::default(), RenderLayers::layer(LAYER)));
        assert_eq!(draws(&mut app, camera, 5), again);
    }

    #[test]
    fn a_picture_draws_while_a_pipeline_is_being_made() {
        let settled = Settled::default();
        let mut app = app(&settled);
        let camera = camera(&mut app);
        assert_eq!(draws(&mut app, camera, 5), [true; 5]);
        // Settled: the frames left of the last unsettled one.
        settled.tell(true);
        assert_eq!(draws(&mut app, camera, 4), [true, true, true, false]);
    }

    /// A window of `width` × `height` pixels, the game's primary.
    fn window(app: &mut App, (width, height): (u32, u32)) -> Entity {
        app.world_mut()
            .spawn((
                Window {
                    resolution: bevy::window::WindowResolution::new(width, height)
                        .with_scale_factor_override(1.0),
                    ..default()
                },
                PrimaryWindow,
            ))
            .id()
    }

    /// Gives the window `width` × `height` pixels.
    fn resize(app: &mut App, window: Entity, (width, height): (u32, u32)) {
        if let Some(mut window) = app.world_mut().get_mut::<Window>(window) {
            window.resolution.set_physical_resolution(width, height);
        }
    }

    /// A picture of `picture`, `size` in the original's pixels, made in a window of `height`
    /// rows, and the node that shows it: its camera, its target and its material.
    fn picture(
        app: &mut App,
        (picture, size): (WindowPixels, Vec2),
        height: f32,
    ) -> (Entity, Handle<Image>, Handle<Layer>) {
        let world = app.world_mut();
        let made = target(pixels(size, height / SCREEN.y));
        let image = world.resource_mut::<Assets<Image>>().add(made);
        let layer = world.resource_mut::<Assets<Layer>>().add(Layer {
            figures: image.clone(),
        });
        world.spawn(MaterialNode(layer.clone()));
        let camera = world
            .spawn(super::camera(image.clone(), (picture, size), (-3, LAYER)))
            .id();
        (camera, image, layer)
    }

    /// The size of the image `image`.
    fn size_of(app: &App, image: &Handle<Image>) -> Option<UVec2> {
        app.world()
            .resource::<Assets<Image>>()
            .get(image)
            .map(Image::size)
    }

    /// Whether the layer `layer` was told changed since the last call.
    fn told(app: &mut App, layer: &Handle<Layer>) -> bool {
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<Layer>>>()
            .drain()
            .any(|event| event.is_modified(layer))
    }

    /// The width and height of a camera's fixed projection, and its place's x and y.
    fn view_of(app: &App, camera: Entity) -> Option<(Vec2, Vec2)> {
        let projection = app.world().get::<Projection>(camera)?;
        let place = app.world().get::<Transform>(camera)?;
        match projection {
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::Fixed { width, height },
                ..
            }) => Some((Vec2::new(*width, *height), place.translation.truncate())),
            _ => None,
        }
    }

    #[test]
    fn a_pictures_target_follows_the_windows_size() {
        let settled = Settled::default();
        settled.tell(true);
        let mut app = app(&settled);
        let window = window(&mut app, (1920, 1080));
        // The weapon sets' picture of the bar (#2697): 400 × 128 of the original's pixels.
        let size = Vec2::new(400.0, 128.0);
        let (camera, image, layer) = picture(&mut app, (WindowPixels::Part(size), size), 1080.0);
        app.update();
        told(&mut app, &layer);
        assert_eq!(size_of(&app, &image), Some(UVec2::new(563, 180)));
        // The full screen: the target made again at its scale, the material told, the view kept.
        resize(&mut app, window, (3840, 2160));
        app.update();
        assert_eq!(size_of(&app, &image), Some(UVec2::new(1125, 360)));
        assert!(told(&mut app, &layer));
        assert_eq!(view_of(&app, camera), Some((size, Vec2::new(200.0, -64.0))));
        // Nothing changed: nothing made again.
        app.update();
        assert!(!told(&mut app, &layer));
        // A minimised window, of no rows, leaves it.
        resize(&mut app, window, (0, 0));
        app.update();
        assert_eq!(size_of(&app, &image), Some(UVec2::new(1125, 360)));
    }

    #[test]
    fn the_held_items_picture_covers_the_whole_window_as_it_changes() {
        let settled = Settled::default();
        settled.tell(true);
        let mut app = app(&settled);
        let window = window(&mut app, (1920, 1080));
        let made = Vec2::new(1920.0, 1080.0) * SCREEN.y / 1080.0;
        let (camera, image, _) = picture(&mut app, (WindowPixels::Whole, made), 1080.0);
        app.update();
        assert_eq!(size_of(&app, &image), Some(UVec2::new(1920, 1080)));
        // A window of 5:4: the original's 768 rows, 960 columns, over its pixels.
        resize(&mut app, window, (1280, 1024));
        app.update();
        assert_eq!(size_of(&app, &image), Some(UVec2::new(1280, 1024)));
        assert_eq!(
            view_of(&app, camera),
            Some((Vec2::new(960.0, 768.0), Vec2::new(480.0, -384.0)))
        );
    }
}
