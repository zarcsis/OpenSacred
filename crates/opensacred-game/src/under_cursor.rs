//! What lies under the cursor (#2873; docs/re/cursor.md, "The chooser's tests, in order"): at each
//! frame every object that the last frame drew, shown and in view, leaves its rectangle on the
//! screen, in the original's pixels ([`View`]): a living creature its type's box about its feet
//! (`opensacred_rules::pick::creature_rect`), a ridden horse's halved; the dead, the containers,
//! the doors and the specials their models' bounds (`vertices::ModelPoints::bounds_points`), a
//! container whose model is not drawn yet a chest's; an item the least square about its point; and
//! a scripts' event of a box its box. Of them `opensacred_rules::pick::choose` takes one
//! ([`UnderCursor`]): of the first class that holds the cursor, the hostile creatures, then the
//! items and the events, then the rest, or the items first while Alt is held, the one whose middle
//! lies nearest the cursor, none where it lies 100 pixels away; with none taken and Alt held, the
//! last label under the cursor (`labels::Shown`, `0x6270A0`). The modules that take a click or show
//! a look on the world read it: the hero's attacks, its arts, its talks, its pick-ups, the doors,
//! the containers, the horses, the events and the cursor's look.

use bevy::prelude::*;
use opensacred_rules::pick::{self, Class, Rect};

use crate::game::AppState;

/// The object under the cursor that the pick took in this frame, if any.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnderCursor(pub Option<Entity>);

/// How far beyond the screen an object's rectangle still goes into the list (the push's margin,
/// `0x62FAD5`).
pub(crate) const BEYOND: f32 = 200.0;
/// The bounds of a model that the original takes for wrong and replaces by a box of 10 about its
/// point (`0x44D19x`): wider than 1,500 across, or both its ends of an axis on the ground farther
/// than 400 from the point to one side.
const WIDEST: f32 = 1500.0;
const FARTHEST: f32 = 400.0;
const FALLBACK: f32 = 10.0;
/// A container's box about its point on the screen at the zoom 1, which the original takes of its
/// model's bounds (`+0xD8` to `+0xEC`): half its width and its height above its point
/// (**estimate**, measured 2026-11-02 beside the original at the Seraphim's start's chest), until
/// its model is drawn.
const CONTAINER_BOX: Vec2 = Vec2::new(35.0, 57.0);
/// How far a hidden stash stands above its cell's ground, its box's point on the ground below it
/// (`stashes`).
const STASH_ABOVE: f32 = crate::stashes::PICKED_ABOVE;

/// The world box of a model of the points `points` about its place `at` (`0x44CF20`, the bounds
/// that its draw grows from its meshes' vertices, `vertices::ModelPoints::bounds_points`): their
/// least and most, unless the original takes them for wrong ([`WIDEST`], [`FARTHEST`]); none
/// for a model of no points, not drawn yet.
pub(crate) fn model_box(points: &[Vec3], at: Vec3) -> Option<(Vec3, Vec3)> {
    let first = *points.first()?;
    let (low, high) = points
        .iter()
        .fold((first, first), |(low, high), &p| (low.min(p), high.max(p)));
    let off = |low: f32, high: f32, at: f32| low - at > FARTHEST || at - high > FARTHEST;
    if high.x - low.x > WIDEST || off(low.x, high.x, at.x) || off(low.z, high.z, at.z) {
        return Some((at - Vec3::splat(FALLBACK), at + Vec3::splat(FALLBACK)));
    }
    Some((low, high))
}

/// The screen as the rectangles take it: the eye's camera and its place, the world's scale on it
/// (2 shows twice the world, `iso::zoom`), the interface's scale of the original's pixels, and the
/// viewport's size.
pub(crate) struct View<'a> {
    camera: &'a Camera,
    eye: &'a GlobalTransform,
    zoom: f32,
    scale: f32,
    size: Vec2,
}

impl<'a> View<'a> {
    /// The view of the eye `camera` at `eye` of the projection `projection`, the interface's scale
    /// `scale`.
    pub(crate) fn new(
        (camera, eye, projection): (&'a Camera, &'a GlobalTransform, &Projection),
        scale: f32,
    ) -> Self {
        let zoom = match projection {
            Projection::Orthographic(ortho) => ortho.scale.max(0.01),
            _ => 1.0,
        };
        Self {
            camera,
            eye,
            zoom,
            scale: scale.max(0.01),
            size: camera.logical_viewport_size().unwrap_or(Vec2::ZERO),
        }
    }

    /// The window's logical point `at` in the original's pixels.
    pub(crate) fn pixels(&self, at: Vec2) -> Vec2 {
        at / self.scale
    }

    /// The place of the world's `at` on the screen in the original's pixels, none beyond the
    /// screen by more than [`BEYOND`].
    pub(crate) fn point(&self, at: Vec3) -> Option<Vec2> {
        let point = self.camera.world_to_viewport(self.eye, at).ok()?;
        let near = point.cmpge(Vec2::splat(-BEYOND)).all() && point.cmplt(self.size + BEYOND).all();
        near.then(|| point / self.scale)
    }

    /// A living creature's rectangle, of the type `kind` standing at `at`, a ridden horse's where
    /// `ridden` (`opensacred_rules::pick::creature_rect`).
    pub(crate) fn creature(&self, at: Vec3, kind: u32, ridden: bool) -> Option<Rect> {
        let feet = self.point(at)?;
        Some(pick::creature_rect(
            feet.to_array(),
            kind,
            self.zoom,
            ridden,
        ))
    }

    /// An item's rectangle: the least square about its point.
    pub(crate) fn item(&self, at: Vec3) -> Option<Rect> {
        let point = self.point(at)?.to_array();
        Some(Rect::between(point, point).held())
    }

    /// A model's rectangle, of the bounds of the points `points` about its place `at`: the
    /// screen's box of its bounds' eight corners (`0x44CF20`, `0x623B60`), its sides held at the
    /// least; none for a model not drawn yet.
    pub(crate) fn model(&self, points: &[Vec3], at: Vec3) -> Option<Rect> {
        self.point(at)?;
        let (low, high) = model_box(points, at)?;
        let corners = (0..8).filter_map(|k: u8| {
            let pick = |bit: u8, low: f32, high: f32| if k & bit == 0 { low } else { high };
            let corner = Vec3::new(
                pick(1, low.x, high.x),
                pick(2, low.y, high.y),
                pick(4, low.z, high.z),
            );
            self.camera.world_to_viewport(self.eye, corner).ok()
        });
        let (min, max) = corners.fold(
            (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
            |(min, max), p| (min.min(p), max.max(p)),
        );
        (min.x <= max.x).then(|| {
            Rect::between((min / self.scale).to_array(), (max / self.scale).to_array()).held()
        })
    }

    /// A container's rectangle before its model is drawn: a chest's about its point.
    pub(crate) fn chest(&self, at: Vec3) -> Option<Rect> {
        let point = self.point(at)?;
        let low = point - CONTAINER_BOX / self.zoom;
        let high = point + Vec2::new(CONTAINER_BOX.x, 0.0) / self.zoom;
        Some(Rect::between(low.to_array(), high.to_array()).held())
    }

    /// A scripts' event's rectangle, its box `boxed` about its cell's point below `at`, at the zoom
    /// 1 in pixels (`0x62CF20`).
    pub(crate) fn event(&self, at: Vec3, boxed: [i16; 4]) -> Option<Rect> {
        let point = self.point(at - Vec3::Y * STASH_ABOVE)?;
        Some(crate::stashes::box_rect(point, boxed, self.zoom))
    }
}

/// Whether an object is shown: not hidden, as a floor that its building does not show hides its
/// objects (`buildings`).
fn shown(visibility: Option<&InheritedVisibility>) -> bool {
    visibility.is_none_or(|visibility| visibility.get())
}

/// An object's place and whether it is shown.
type Place = (
    Entity,
    &'static GlobalTransform,
    Option<&'static InheritedVisibility>,
);

/// The creatures as the pick takes them: their place, their mind, their health, whether they
/// die and whether they carry a rider.
type Creatures<'w, 's> = Query<
    'w,
    's,
    (
        Place,
        &'static crate::ai::Mind,
        &'static crate::fight::Health,
        Has<crate::npcs::Dies>,
        Has<crate::riding::Ridden>,
        Has<crate::dryad_ghosts::Dummy>,
    ),
    Without<crate::hero::Body>,
>;

/// The other objects drawn: the hero, the items lying, the containers, the doors, the dead lying,
/// the specials and the scripts' events.
type Objects<'w, 's> = (
    Query<'w, 's, Place, With<crate::hero::Body>>,
    Query<'w, 's, Place, With<crate::drops::Lying>>,
    Query<'w, 's, Place, With<crate::containers::Container>>,
    Query<'w, 's, Place, With<crate::npcs::Door>>,
    Query<'w, 's, (Place, &'static crate::npcs::Corpse), Without<crate::ai::Mind>>,
    Query<'w, 's, Place, With<crate::npcs::Special>>,
    Query<
        'w,
        's,
        (
            Entity,
            &'static GlobalTransform,
            &'static crate::stashes::Stash,
        ),
    >,
);

/// Takes the object under the cursor ([`UnderCursor`]) from the rectangles of the objects in view.
fn choose(
    mut picked: ResMut<UnderCursor>,
    (pointer, keys, ui_scale, labels): (
        Res<crate::hud::Pointer>,
        crate::controls::Keys,
        Res<UiScale>,
        Res<crate::labels::Shown>,
    ),
    camera: Single<(&Camera, &GlobalTransform, &Projection), crate::iso::Eye<With<Camera>>>,
    (points, meshes, arts): (
        crate::vertices::ModelPoints,
        Res<Assets<Mesh>>,
        Option<Res<crate::arts::HeroArts>>,
    ),
    creatures: Creatures,
    (heroes, items, containers, doors, corpses, specials, events): Objects,
) {
    let Some(cursor) = pointer.0 else {
        picked.set_if_neq(UnderCursor(None));
        return;
    };
    let view = View::new(*camera, ui_scale.0);
    let model = |entity: Entity, at: Vec3| view.model(&points.bounds_points(entity, &meshes), at);
    let mut drawn: Vec<(Entity, Class, Rect)> = Vec::new();
    for ((entity, at, visibility), mind, health, dies, ridden, dummy) in &creatures {
        let alive = !dies && health.current > 0;
        // A dryad ghost's dummy draws nothing and is picked while it lives (`0x44CF20`).
        if !shown(visibility) && !(dummy && alive) {
            continue;
        }
        let at = at.translation();
        let class = if alive && mind.hostile() {
            Class::Foe
        } else {
            Class::Other
        };
        // A dying creature's box of its model as it falls (`+0xB0` bit 2, `0x44D08A`).
        let rect = if alive {
            view.creature(at, mind.creature, ridden)
        } else {
            model(entity, at).or_else(|| view.creature(at, mind.creature, false))
        };
        drawn.extend(rect.map(|rect| (entity, class, rect)));
    }
    let class = arts.as_deref().map_or(1, |arts| arts.kind);
    for (entity, at, _) in heroes.iter().filter(|&(.., visibility)| shown(visibility)) {
        drawn.extend(
            view.creature(at.translation(), class, false)
                .map(|rect| (entity, Class::Other, rect)),
        );
    }
    for (entity, at, _) in items.iter().filter(|&(.., visibility)| shown(visibility)) {
        drawn.extend(
            view.item(at.translation())
                .map(|rect| (entity, Class::Item, rect)),
        );
    }
    for (entity, at, _) in containers
        .iter()
        .filter(|&(.., visibility)| shown(visibility))
    {
        let at = at.translation();
        let rect = model(entity, at).or_else(|| view.chest(at));
        drawn.extend(rect.map(|rect| (entity, Class::Other, rect)));
    }
    for (entity, at, _) in doors
        .iter()
        .chain(&specials)
        .filter(|&(.., visibility)| shown(visibility))
    {
        drawn.extend(model(entity, at.translation()).map(|rect| (entity, Class::Other, rect)));
    }
    for ((entity, at, visibility), corpse) in &corpses {
        if !shown(visibility) {
            continue;
        }
        let at = at.translation();
        let rect = model(entity, at).or_else(|| view.creature(at, corpse.creature, false));
        drawn.extend(rect.map(|rect| (entity, Class::Other, rect)));
    }
    for (entity, at, stash) in &events {
        let rect = stash
            .boxed
            .and_then(|boxed| view.event(at.translation(), boxed));
        drawn.extend(rect.map(|rect| (entity, Class::Item, rect)));
    }
    let alt = crate::labels::held(&keys);
    let mut under = pick::choose(&drawn, view.pixels(cursor).to_array(), alt);
    if under.is_none() && alt {
        under = labels.last_under(cursor);
    }
    picked.set_if_neq(UnderCursor(under));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_models_box_is_its_points_bounds_unless_they_lie_wrong() {
        let at = Vec3::new(1000.0, 0.0, 2000.0);
        let points = [
            Vec3::new(990.0, 0.0, 1980.0),
            Vec3::new(1030.0, 90.0, 2010.0),
        ];
        assert_eq!(
            model_box(&points, at),
            Some((
                Vec3::new(990.0, 0.0, 1980.0),
                Vec3::new(1030.0, 90.0, 2010.0)
            ))
        );
        // Both ends 400 off to one side: a box of 10 about the point.
        let off = [
            Vec3::new(1401.0, 0.0, 2000.0),
            Vec3::new(1500.0, 50.0, 2000.0),
        ];
        assert_eq!(
            model_box(&off, at),
            Some((at - Vec3::splat(10.0), at + Vec3::splat(10.0)))
        );
        assert_eq!(model_box(&[], at), None);
    }
}

/// Clears the pick outside a game.
fn clear(mut picked: ResMut<UnderCursor>) {
    picked.set_if_neq(UnderCursor(None));
}

/// The step of `PreUpdate` that takes the pick, after the pointer's place and the frame's hold
/// (`frame::Hold`), on the places that the last frame drew.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Picking;

/// Takes the object under the cursor at each frame of a game.
pub struct UnderCursorPlugin;

impl Plugin for UnderCursorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UnderCursor>()
            .configure_sets(PreUpdate, Picking.after(crate::frame::Hold))
            .add_systems(
                PreUpdate,
                (
                    choose.run_if(in_state(AppState::InGame)),
                    clear.run_if(not(in_state(AppState::InGame))),
                )
                    .in_set(Picking),
            );
    }
}
