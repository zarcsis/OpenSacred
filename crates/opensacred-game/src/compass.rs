//! The bar's compass (#1462; docs/re/interface.md, "The compass" and "The markers"): the needle,
//! the sword of image 101 turned about (512, 710), points at the newest main quest's compass, and
//! the inner pin, image 136 on an ellipse of 30 × 15 about it, at the newest side quest's, fading
//! within its radius (`opensacred_rules::quests`); a marker set anew starts a sonar ring at
//! (512, 711) with the compass's sound. A target is a cell, or a creature by its name in the
//! scripts (`npcs::ScriptName`); the direction is the target's place less the hero's as the
//! screen shows it, φ the angle clockwise from up. The outer pin, image 137 on an ellipse of
//! 58 × 29, points at the world map's pin (#1569). Not followed: the blinking pin of
//! `GroupIsDead`'s creature, objects named by `CreateObj`, and the way across a dungeon's mouth.

use bevy::prelude::*;
use opensacred_rules::quests::{Compass, Marker, Target, side_alpha};

use crate::bar::{self, BAR, BarRoot, UiImages};
use crate::game::AppState;
use crate::hero::Body;
use crate::low_health::{SONAR, SONAR_FIRST, Sonar, sonar_next, sonar_node};
use crate::npcs::ScriptName;
use crate::{hud, iso, paths, scripts};

/// The needle's, the inner pin's and the outer pin's images.
const NEEDLE: u32 = 101;
const PIN: u32 = 136;
const OUTER_PIN: u32 = 137;
/// The compass's centre on the screen of 1024 × 768, and the inner pins' ellipse's half axes.
const CENTRE: Vec2 = Vec2::new(512.0, 710.0);
const INNER: Vec2 = Vec2::new(30.0, 15.0);
const OUTER: Vec2 = Vec2::new(58.0, 29.0);
/// The sonar's centre, its tint `0xDDCCAA`, and how many run at once.
const SONAR_CENTRE: Vec2 = Vec2::new(512.0, 711.0);
const SONAR_TINT: Vec3 = Vec3::new(221.0 / 255.0, 204.0 / 255.0, 170.0 / 255.0);
const SONARS: usize = 4;
/// The compass's sound, the event (9, `0x46`): the sound id 7027 (docs/re/sound.md).
const SOUND: u32 = 7027;

/// The needle, or a pin of a marker.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
struct Pointer(Marker);

/// The outer pin, toward the world map's pin (the marker 2, #1569).
#[derive(Component)]
struct OuterPin;

/// A sonar's ring, while it spreads.
#[derive(Component, Default)]
struct Ring(Option<f32>);

/// The needle and the pin, hidden, and the sonars' rings on the bar.
fn spawn(
    mut commands: Commands,
    images: Res<UiImages>,
    assets: Res<AssetServer>,
    mut sonars: ResMut<Assets<Sonar>>,
    bars: Query<Entity, With<BarRoot>>,
) {
    let Ok(bar) = bars.single() else {
        return;
    };
    for (marker, index) in [(Marker::Main, NEEDLE), (Marker::Side, PIN)] {
        let size = images.size(index);
        let (image, node) = bar::piece(&images, &assets, index, CENTRE - size / 2.0, BAR.min);
        commands.entity(bar).with_child((
            Pointer(marker),
            image,
            node,
            UiTransform::IDENTITY,
            Visibility::Hidden,
            ZIndex(2),
            Pickable::IGNORE,
        ));
    }
    let size = images.size(OUTER_PIN);
    let (image, node) = bar::piece(&images, &assets, OUTER_PIN, CENTRE - size / 2.0, BAR.min);
    commands.entity(bar).with_child((
        OuterPin,
        image,
        node,
        UiTransform::IDENTITY,
        Visibility::Hidden,
        ZIndex(2),
        Pickable::IGNORE,
    ));
    let Some(file) = images.file(SONAR) else {
        return;
    };
    let texture = hud::image(&assets, file.to_owned());
    for _ in 0..SONARS {
        commands.entity(bar).with_child((
            Ring::default(),
            bevy::ui_render::prelude::MaterialNode(sonars.add(Sonar::new(texture.clone()))),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            Visibility::Hidden,
            ZIndex(3),
            Pickable::IGNORE,
        ));
    }
}

/// The angle φ of the screen's direction `(x, y)`, y down: clockwise from up.
fn angle(direction: Vec2) -> f32 {
    direction.x.atan2(-direction.y)
}

/// The pin's place on its ellipse at the angle φ, its centre's.
fn on_ellipse(phi: f32, axes: Vec2) -> Vec2 {
    CENTRE + Vec2::new(axes.x * phi.sin(), -axes.y * phi.cos())
}

/// Where a compass's target stands on the ground (`x`, `z`), a creature's where it is now.
pub(crate) fn target_of(
    compass: &Compass,
    named: &Query<(&ScriptName, &GlobalTransform)>,
) -> Option<Vec2> {
    match &compass.target {
        Target::Cell(x, y) => Some(paths::centre((*x, *y))),
        Target::Named(name) => named
            .iter()
            .find(|(script, _)| script.0.eq_ignore_ascii_case(name))
            .map(|(_, at)| at.translation().xz()),
    }
}

/// Turns the needle and the pin toward their markers' targets.
fn point(
    scripts: Option<NonSend<scripts::Live>>,
    heroes: Query<&GlobalTransform, With<Body>>,
    named: Query<(&ScriptName, &GlobalTransform)>,
    camera: Single<(&Camera, &GlobalTransform), iso::Eye<With<Camera>>>,
    images: Res<UiImages>,
    mut pointers: Query<(
        &Pointer,
        &mut Node,
        &mut UiTransform,
        &mut ImageNode,
        &mut Visibility,
    )>,
) {
    let (camera, eye) = *camera;
    let hero = heroes.iter().next().map(GlobalTransform::translation);
    for (pointer, mut node, mut transform, mut image, mut visibility) in &mut pointers {
        let shown = scripts.as_ref().zip(hero).and_then(|(live, hero)| {
            let compass = live.marker(pointer.0)?;
            let target = target_of(compass, &named)?;
            let place = Vec3::new(target.x, hero.y, target.y);
            let screen = camera.world_to_viewport(eye, place).ok()?
                - camera.world_to_viewport(eye, hero).ok()?;
            let distance = crate::making::whole_distance(place, hero);
            Some((angle(screen), distance, compass.radius))
        });
        let Some((phi, distance, radius)) = shown else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        visibility.set_if_neq(Visibility::Inherited);
        transform.rotation = Rot2::radians(phi);
        if pointer.0 == Marker::Side {
            let size = images.size(PIN);
            let at = on_ellipse(phi, INNER) - size / 2.0 - BAR.min;
            node.left = Val::Px(at.x);
            node.top = Val::Px(at.y);
            // The side quest's pin fades within its radius (`0x4A5980`).
            let alpha = side_alpha(distance, radius);
            image.color = Color::WHITE.with_alpha(f32::from(alpha) / 255.0);
        }
    }
}

/// Turns the outer pin on its ellipse of 58 × 29 toward the world map's pin, the marker 2
/// (`0x4A63E0`), hidden while no pin is placed.
fn point_outer(
    map: Res<crate::world_map::Map>,
    heroes: Query<&GlobalTransform, With<Body>>,
    camera: Single<(&Camera, &GlobalTransform), iso::Eye<With<Camera>>>,
    images: Res<UiImages>,
    mut pins: Query<(&mut Node, &mut UiTransform, &mut Visibility), With<OuterPin>>,
) {
    let (camera, eye) = *camera;
    let hero = heroes.iter().next().map(GlobalTransform::translation);
    let phi = map.pin_target().zip(hero).and_then(|(target, hero)| {
        let place = Vec3::new(target.x, hero.y, target.y);
        let screen = camera.world_to_viewport(eye, place).ok()?
            - camera.world_to_viewport(eye, hero).ok()?;
        Some(angle(screen))
    });
    for (mut node, mut transform, mut visibility) in &mut pins {
        let Some(phi) = phi else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        visibility.set_if_neq(Visibility::Inherited);
        transform.rotation = Rot2::radians(phi);
        let at = on_ellipse(phi, OUTER) - images.size(OUTER_PIN) / 2.0 - BAR.min;
        node.left = Val::Px(at.x);
        node.top = Val::Px(at.y);
    }
}

/// A marker set anew starts a sonar, with the compass's sound; the rings spread.
fn sonar(
    time: Res<Time>,
    scripts: Option<NonSendMut<scripts::Live>>,
    mut rings: Query<(
        &mut Ring,
        &bevy::ui_render::prelude::MaterialNode<Sonar>,
        &mut Node,
        &mut Visibility,
    )>,
    mut sonars: ResMut<Assets<Sonar>>,
    mut sounds: MessageWriter<crate::sfx::Play>,
) {
    if scripts.is_some_and(|mut live| live.take_fresh_marker()) {
        if let Some((mut ring, ..)) = rings.iter_mut().find(|(ring, ..)| ring.0.is_none()) {
            ring.0 = Some(SONAR_FIRST);
        }
        sounds.write(crate::sfx::Play {
            sound: crate::sfx::Of::Id(SOUND),
            at: None,
            rate: (1.0, crate::sfx::Roll::None),
        });
    }
    for (mut ring, material, mut node, mut visibility) in &mut rings {
        let Some(radius) = ring.0 else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        visibility.set_if_neq(Visibility::Inherited);
        sonar_node(&mut node, SONAR_CENTRE - BAR.min, radius);
        if let Some(mut material) = sonars.get_mut(&material.0) {
            material.spread(radius, SONAR_TINT);
        }
        ring.0 = sonar_next(radius, time.delta_secs());
    }
}

/// The bar's compass.
pub struct CompassPlugin;

impl Plugin for CompassPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), spawn.after(bar::spawn))
            .add_systems(
                Update,
                (point, point_outer, sonar)
                    .after(bar::fill)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_angle_runs_clockwise_from_up() {
        assert!(angle(Vec2::new(0.0, -1.0)).abs() < 1e-6);
        assert!((angle(Vec2::new(1.0, 0.0)) - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!((angle(Vec2::new(0.0, 1.0)).abs() - std::f32::consts::PI).abs() < 1e-6);
        // Up, the pin stands above the centre; to the right, right of it.
        assert_eq!(on_ellipse(0.0, INNER), CENTRE - Vec2::new(0.0, 15.0));
        let right = on_ellipse(std::f32::consts::FRAC_PI_2, INNER);
        assert!((right - (CENTRE + Vec2::new(30.0, 0.0))).length() < 1e-4);
    }
}
