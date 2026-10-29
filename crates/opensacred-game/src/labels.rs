//! The items' labels while Alt is held (#1409; docs/re/items.md, "On the ground"): over each item
//! lying in view its name, or for gold its amount and its type's name, white on a dark
//! translucent panel (`0xCF303030`), laid out so that no two overlap
//! (`cWorldView_drawOverheadLabels`, `0x627180`, `cLabelLayout`); a left click on a label takes its
//! item (`0x6270A0`, `pickup::pick`), and with Alt held an item comes before a foe under the
//! cursor (`attack::aim`).
//!
//! Not followed yet: the labels over the creatures and the objects, the items' colours (#1273),
//! the highlight of the label under the cursor, and the hand over a label (`cursors`).

use std::collections::HashMap;

use bevy::prelude::*;
use opensacred_rules::drops::Kind;

use crate::drops::{Lying, Tables};
use crate::game::{AppState, FONT};
use crate::lines::Texts;
use crate::npcs::TextName;

/// The labels' panel (`0xCF303030`), their text's size and line and the panel's margin about it,
/// in the original's pixels (**estimate**, seen 2026-10-29: a potion's name 167 pixels wide on a
/// panel 18 high, the labels of items on one spot 22 pixels apart).
const PANEL: Color = Color::srgba_u8(0x30, 0x30, 0x30, 0xCF);
const SIZE: f32 = 18.0;
const LINE: f32 = 16.0;
const MARGIN: Vec2 = Vec2::new(3.0, 1.0);
/// A label's height, its line and the panel's margins, and the gap between two labels laid one
/// under the other.
const HEIGHT: f32 = LINE + 2.0 * MARGIN.y;
const GAP: f32 = 4.0;
/// The width of a character of a label not yet laid out, before its text is measured
/// (**estimate**).
const CHARACTER: f32 = 8.0;

/// The labels shown, each its item and its rectangle in the window's logical pixels, which a click
/// on one takes (`pickup::pick`).
#[derive(Resource, Default, Debug)]
pub struct Shown(Vec<(Entity, Rect)>);

impl Shown {
    /// The item whose label lies under the point `at` of the window, if any.
    pub fn under(&self, at: Vec2) -> Option<Entity> {
        self.0
            .iter()
            .find(|(_, rect)| rect.contains(at))
            .map(|&(item, _)| item)
    }
}

/// Whether Alt is held (`GetAsyncKeyState` of `VK_MENU`, the world view's flag `0x4000`).
pub fn held(keys: &ButtonInput<KeyCode>) -> bool {
    keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight])
}

/// A label's node, by the item it names.
#[derive(Component)]
struct Label(Entity);

/// The text of an item's label: its name (`cursors::item_name`), or for gold the amount and its
/// pile's name (`0x627180`).
fn text(
    (texts, tables): (&Texts, Option<&Tables>),
    lying: &Lying,
    name: Option<&TextName>,
) -> Option<String> {
    let dropped = &lying.dropped;
    let named =
        crate::cursors::item_name(texts, tables, dropped, name.map(|name| name.0.as_str()))?;
    Some(if dropped.kind == Kind::Gold {
        format!("{} {named}", dropped.amount)
    } else {
        named.to_owned()
    })
}

/// Lays out labels of the sizes `sizes`, each wanted with its left edge on its point of `points`,
/// centred on it upright, so that no two overlap (`cLabelLayout`): by their points from the top
/// down and from the left, each moved down under any label already laid that it would cover
/// (**estimate**: the layout is not read; seen, the labels of items on one spot stack downward,
/// their left edges together).
fn lay_out(points: &[Vec2], sizes: &[Vec2]) -> Vec<Rect> {
    let mut order: Vec<usize> = (0..points.len().min(sizes.len())).collect();
    order.sort_by(|&a, &b| {
        (points[a].y, points[a].x)
            .partial_cmp(&(points[b].y, points[b].x))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut laid = vec![Rect::default(); order.len()];
    let mut placed: Vec<Rect> = Vec::with_capacity(order.len());
    for index in order {
        let corner = points[index] - Vec2::new(0.0, sizes[index].y / 2.0);
        let mut rect = Rect::from_corners(corner, corner + sizes[index]);
        while let Some(below) = placed
            .iter()
            .filter(|other| !other.intersect(rect).is_empty())
            .map(|other| other.max.y)
            .reduce(f32::max)
        {
            rect.min.y = below + GAP;
            rect.max.y = rect.min.y + sizes[index].y;
        }
        placed.push(rect);
        laid[index] = rect;
    }
    laid
}

/// Shows the labels of the items lying in view while Alt is held, and takes them away as it is
/// let go.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn label(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    (texts, tables): (Option<Res<Texts>>, Option<Res<Tables>>),
    (assets, ui_scale): (Res<AssetServer>, Res<UiScale>),
    camera: Single<(&Camera, &GlobalTransform), crate::iso::Eye<With<Camera>>>,
    items: Query<(Entity, &GlobalTransform, &Lying, Option<&TextName>)>,
    mut labels: Query<(Entity, &Label, &mut Node, &mut Text, &ComputedNode)>,
    mut shown: ResMut<Shown>,
) {
    let wanted: Vec<(Entity, Vec2, String)> = match texts.as_deref() {
        Some(texts) if held(&keys) => {
            let (camera, view) = *camera;
            let size = camera.logical_viewport_size().unwrap_or(Vec2::ZERO);
            items
                .iter()
                .filter(|(_, _, lying, _)| lying.resting)
                .filter_map(|(item, at, lying, name)| {
                    let point = camera.world_to_viewport(view, at.translation()).ok()?;
                    let inside = point.cmpge(Vec2::ZERO).all() && point.cmplt(size).all();
                    inside.then_some(())?;
                    Some((item, point, text((texts, tables.as_deref()), lying, name)?))
                })
                .collect()
        }
        _ => Vec::new(),
    };
    let scale = ui_scale.0.max(0.01);
    // The sizes of the labels laid out before, in the original's pixels (the node's inverse scale
    // takes the window's and `UiScale` both); a new one's estimated.
    let measured: HashMap<Entity, Vec2> = labels
        .iter()
        .map(|(_, label, _, _, computed)| {
            (label.0, computed.size() * computed.inverse_scale_factor())
        })
        .collect();
    let points: Vec<Vec2> = wanted.iter().map(|(_, point, _)| *point / scale).collect();
    let sizes: Vec<Vec2> = wanted
        .iter()
        .map(|(item, _, text)| {
            measured
                .get(item)
                .copied()
                .filter(|size| size.x > 0.0)
                .unwrap_or_else(|| {
                    let characters = f32::from(u16::try_from(text.chars().count()).unwrap_or(0));
                    Vec2::new(characters * CHARACTER + 2.0 * MARGIN.x, HEIGHT)
                })
        })
        .collect();
    let rects = lay_out(&points, &sizes);
    let mut nodes: HashMap<Entity, Entity> = HashMap::new();
    for (entity, label, ..) in &labels {
        if wanted.iter().any(|(item, ..)| *item == label.0) {
            nodes.insert(label.0, entity);
        } else {
            commands.entity(entity).despawn();
        }
    }
    shown.0.clear();
    for ((item, _, name), rect) in wanted.iter().zip(&rects) {
        shown.0.push((
            *item,
            Rect::from_corners(rect.min * scale, rect.max * scale),
        ));
        if let Some(&node) = nodes.get(item)
            && let Ok((_, _, mut at, mut words, _)) = labels.get_mut(node)
        {
            at.left = Val::Px(rect.min.x);
            at.top = Val::Px(rect.min.y);
            if words.0 != *name {
                name.clone_into(&mut words.0);
            }
            continue;
        }
        commands.spawn((
            Label(*item),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(rect.min.x),
                top: Val::Px(rect.min.y),
                padding: UiRect::axes(Val::Px(MARGIN.x), Val::Px(MARGIN.y)),
                ..default()
            },
            BackgroundColor(PANEL),
            Text::new(name.clone()),
            TextFont {
                font: assets.load(FONT).into(),
                font_size: FontSize::Px(SIZE),
                ..default()
            },
            bevy::text::LineHeight::Px(LINE),
            TextColor(Color::WHITE),
            GlobalZIndex(9),
            Pickable::IGNORE,
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// The items' labels on Alt.
pub struct LabelsPlugin;

impl Plugin for LabelsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shown>().add_systems(
            Update,
            label
                .run_if(in_state(AppState::InGame))
                .before(crate::frame::Control),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_of_one_spot_stack_downward_and_others_stay_where_they_are() {
        let size = Vec2::new(100.0, HEIGHT);
        let points = [
            Vec2::new(500.0, 300.0),
            Vec2::new(500.0, 300.0),
            Vec2::new(505.0, 302.0),
            Vec2::new(200.0, 300.0),
        ];
        let rects = lay_out(&points, &[size; 4]);
        let wanted = |point: Vec2| {
            Rect::from_corners(
                point - Vec2::new(0.0, HEIGHT / 2.0),
                point + Vec2::new(100.0, HEIGHT / 2.0),
            )
        };
        assert_eq!(rects[0], wanted(points[0]));
        // The second of the same spot goes under the first, the third under both, each at its
        // own point's left edge.
        assert_eq!(rects[1].min.y, rects[0].max.y + GAP);
        assert_eq!(rects[2].min.y, rects[1].max.y + GAP);
        assert_eq!(rects[2].min.x, 505.0);
        // One far off keeps its place.
        assert_eq!(rects[3], wanted(points[3]));
    }

    #[test]
    fn a_click_on_a_label_finds_its_item() {
        let item = Entity::PLACEHOLDER;
        let shown = Shown(vec![(item, Rect::new(10.0, 10.0, 60.0, 30.0))]);
        assert_eq!(shown.under(Vec2::new(20.0, 20.0)), Some(item));
        assert_eq!(shown.under(Vec2::new(70.0, 20.0)), None);
    }
}
