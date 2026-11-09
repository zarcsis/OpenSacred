//! The bar's compass (#1462; docs/re/interface.md, "The compass" and "The markers"): the needle,
//! the sword of image 101 turned about (512, 710), points at the newest main quest's compass, and
//! the inner pin, image 136 on an ellipse of 30 × 15 about it, at the newest side quest's, fading
//! within its radius (`opensacred_rules::quests`); a marker set anew starts a sonar ring at
//! (512, 711) with the compass's sound. A target is a cell, or a creature by its name in the
//! scripts (`npcs::ScriptName`); the direction is the target's place less the hero's as the
//! screen shows it, φ the angle clockwise from up. The outer pin, image 137 on an ellipse of
//! 58 × 29, points at the world map's pin (#1569). Not followed: the blinking pin of
//! `GroupIsDead`'s creature, objects named by `CreateObj`, and the way across a dungeon's mouth.
//! A creature that lives away from the world is sought where it stands away (#2406).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use opensacred_rules::quests::{Compass, Marker, Target, side_alpha};
use serde::Serialize;

use crate::bar::{self, BAR, BarRoot, UiImages};
use crate::game::AppState;
use crate::hero::Body;
use crate::low_health::{SONAR, SONAR_FIRST, Sonar, sonar_next, sonar_node};
use crate::npcs::{Npcs, ScriptName};
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

/// The screen's direction (`x` right, `y` down) of the world's direction `direction`, as the
/// camera `eye` turns it: by its rotation alone, not its projection, which refuses a place
/// beyond its far plane, as a target a few sectors away lies (#2406).
fn on_screen(eye: &GlobalTransform, direction: Vec3) -> Vec2 {
    let view = eye.rotation().inverse() * direction;
    Vec2::new(view.x, -view.y)
}

/// The pin's place on its ellipse at the angle φ, its centre's.
fn on_ellipse(phi: f32, axes: Vec2) -> Vec2 {
    CENTRE + Vec2::new(axes.x * phi.sin(), -axes.y * phi.cos())
}

/// What a compass's target is sought by: the creatures in the world by their names in the
/// scripts, and the scripts' creatures living away from it ([`Npcs::away`], #2406). The
/// original's markers look a named target up among the world's objects, which hold the
/// scripts' creatures wherever they stand, and follow its place each frame (`0x4A5980`).
#[derive(SystemParam)]
pub(crate) struct Seek<'w, 's> {
    named: Query<'w, 's, (&'static ScriptName, &'static GlobalTransform)>,
    npcs: Option<Res<'w, Npcs>>,
}

impl Seek<'_, '_> {
    /// Where the creature of the script name `name` stands on the ground (`x`, `z`), and whether
    /// it is in the world: its place there, else the cell where it lives away from it.
    fn creature(&self, name: &str) -> Option<(Vec2, bool)> {
        if let Some((_, at)) = self
            .named
            .iter()
            .find(|(script, _)| script.0.eq_ignore_ascii_case(name))
        {
            return Some((at.translation().xz(), true));
        }
        let (x, y) = self.npcs.as_deref()?.away(name)?;
        let cell = (i32::try_from(x).ok()?, i32::try_from(y).ok()?);
        Some((paths::centre(cell), false))
    }

    /// Where a compass's target stands on the ground (`x`, `z`): a cell's centre, or the named
    /// creature's place ([`Self::creature`]).
    pub(crate) fn target_of(&self, compass: &Compass) -> Option<Vec2> {
        match &compass.target {
            Target::Cell(x, y) => Some(paths::centre((*x, *y))),
            Target::Named(name) => self.creature(name).map(|(at, _)| at),
        }
    }
}

/// Where a marker of the compass points, as the state tells it (#2403): its quest, the target's
/// cell (a creature's where it stands now, none for one killed or not yet made), the
/// creature's name in the scripts where the target is one, and whether it lives away from the
/// world (#2406).
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Aim {
    pub quest: u32,
    pub cell: Option<paths::Cell>,
    pub name: Option<String>,
    pub away: bool,
}

impl Aim {
    fn of(quest: u32, compass: &Compass, seek: &Seek) -> Self {
        let (cell, name, away) = match &compass.target {
            Target::Cell(x, y) => (Some((*x, *y)), None, false),
            Target::Named(name) => {
                let found = seek.creature(name);
                (
                    found.map(|(at, _)| paths::cell_of(at.x, at.y)),
                    Some(name.clone()),
                    found.is_some_and(|(_, world)| !world),
                )
            }
        };
        Self {
            quest,
            cell,
            name,
            away,
        }
    }

    /// The state's words for it: `quest N at X,Y`, the creature's name after the cell.
    pub fn line(&self) -> String {
        let mut line = format!("quest {}", self.quest);
        match (self.cell, &self.name) {
            (Some((x, y)), name) => {
                line.push_str(&format!(" at {x},{y}"));
                if let Some(name) = name {
                    line.push_str(&format!(" {name}"));
                }
                if self.away {
                    line.push_str(" away");
                }
            }
            (None, Some(name)) => line.push_str(&format!(" {name}, nowhere")),
            (None, None) => {}
        }
        line
    }
}

/// The compass's markers' targets now, the needle's and the inner pin's, for the state (#2403).
#[derive(Resource, Serialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct Targets {
    pub main: Option<Aim>,
    pub side: Option<Aim>,
}

/// Keeps [`Targets`] to the markers' quests and their targets' places.
fn aim(scripts: Option<NonSend<scripts::Live>>, seek: Seek, mut targets: ResMut<Targets>) {
    let of = |marker| {
        let (quest, compass) = scripts.as_ref()?.quests().marked(marker)?;
        Some(Aim::of(quest, compass, &seek))
    };
    targets.set_if_neq(Targets {
        main: of(Marker::Main),
        side: of(Marker::Side),
    });
}

/// Turns the needle and the pin toward their markers' targets.
fn point(
    scripts: Option<NonSend<scripts::Live>>,
    heroes: Query<&GlobalTransform, With<Body>>,
    seek: Seek,
    eye: Single<&GlobalTransform, iso::Eye<With<Camera>>>,
    images: Res<UiImages>,
    mut pointers: Query<(
        &Pointer,
        &mut Node,
        &mut UiTransform,
        &mut ImageNode,
        &mut Visibility,
    )>,
) {
    let hero = heroes.iter().next().map(GlobalTransform::translation);
    for (pointer, mut node, mut transform, mut image, mut visibility) in &mut pointers {
        let shown = scripts.as_ref().zip(hero).and_then(|(live, hero)| {
            let compass = live.marker(pointer.0)?;
            let target = seek.target_of(compass)?;
            let place = Vec3::new(target.x, hero.y, target.y);
            let screen = on_screen(&eye, place - hero);
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
    eye: Single<&GlobalTransform, iso::Eye<With<Camera>>>,
    images: Res<UiImages>,
    mut pins: Query<(&mut Node, &mut UiTransform, &mut Visibility), With<OuterPin>>,
) {
    let hero = heroes.iter().next().map(GlobalTransform::translation);
    let phi = map.pin_target().zip(hero).map(|(target, hero)| {
        let place = Vec3::new(target.x, hero.y, target.y);
        angle(on_screen(&eye, place - hero))
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
        app.init_resource::<Targets>()
            .add_systems(OnEnter(AppState::InGame), spawn.after(bar::spawn))
            .add_systems(
                Update,
                (aim, point, point_outer, sonar)
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

    #[test]
    fn a_far_target_turns_by_the_cameras_rotation() {
        // The cells' x runs right and down the screen, their y left and down, however far.
        let eye = GlobalTransform::from(iso::view_of(Vec3::ZERO));
        let step = |x: f32, y: f32| iso::ground(x, y) - iso::ground(0.0, 0.0);
        let x = on_screen(&eye, step(200.0, 0.0));
        let y = on_screen(&eye, step(0.0, 200.0));
        assert!(x.x > 0.0 && x.y > 0.0, "{x}");
        assert!(y.x < 0.0 && y.y > 0.0, "{y}");
        assert!((x.x + y.x).abs() < 1e-2 && (x.y - y.y).abs() < 1e-2);
        assert!((x.x / x.y - 2.0).abs() < 1e-3, "the iso view's 2:1");
    }
}
