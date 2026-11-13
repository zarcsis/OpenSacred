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

use bevy::asset::{AssetEvent, AssetEventSystems, AssetId};
use bevy::camera::RenderTarget;
use bevy::camera::visibility::{RenderLayers, VisibilitySystems};
use bevy::ecs::query::QueryItem;
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::transform::TransformSystems;

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
    (mut standard_events, mut apart_events, mut image_events): (
        MessageReader<AssetEvent<StandardMaterial>>,
        MessageReader<AssetEvent<ApartMaterial>>,
        MessageReader<AssetEvent<Image>>,
    ),
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

/// The items' pictures drawn only when they change.
pub struct ItemPicturesPlugin;

impl Plugin for ItemPicturesPlugin {
    fn build(&self, app: &mut App) {
        // After the frame's moves, shows and assets' changes are told, before the cameras'
        // visible entities are found, which an inactive camera's are not.
        app.add_systems(
            PostUpdate,
            draw_on_change
                .after(AssetEventSystems)
                .after(TransformSystems::Propagate)
                .after(VisibilitySystems::VisibilityPropagate)
                .before(VisibilitySystems::CheckVisibility),
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
}
