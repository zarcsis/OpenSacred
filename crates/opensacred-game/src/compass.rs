//! The bar's compass (#1462; docs/re/interface.md, "The compass" and "The markers"): the needle,
//! the sword of image 101 turned about (512, 710), points at the newest main quest's compass, and
//! the inner pin, image 136 on an ellipse of 30 × 15 about it, at the newest side quest's, whole at
//! any distance, as the original's draw drops the resolver's colours (`opensacred_rules::quests`,
//! #3535); a marker set anew starts a sonar ring at
//! (512, 711) with the compass's sound. A target is a cell, or a creature by its name in the
//! scripts (`npcs::ScriptName`); the direction is the target's place less the hero's turned
//! by 45° alone, not squashed as the isometric view squashes it (`0x4A5980`, #3244), φ the
//! angle clockwise from up. The outer pin, image 137 on an ellipse of
//! 58 × 29, points at the world map's pin (#1569). The marker 3, the creature that a script's
//! `GroupIsDead` found the nearest living member of its group, blinks for 60 s of the real time,
//! every 300 ms the outer pin on the outer ellipse and then the inner pin on the inner one, drawn
//! after the needle and before the other pins (`0x48C860`, `0x6E3F70`; docs/re/interface.md, "The
//! compass", #3104). Where the hero and a marker's target lie on different sides of a
//! dungeon's mouth, every marker points at the way between them, the way out at every call
//! (`opensacred_rules::mouths`, #3419). Not followed: objects named by `CreateObj`.
//! A creature that lives away from the world is sought where it stands away (#2406).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use opensacred_rules::quests::{Compass, Marker, Target};
use serde::Serialize;

use crate::bar::{self, BAR, BarRoot, UiImages};
use crate::game::AppState;
use crate::hero::Body;
use crate::low_health::{SONAR, SONAR_FIRST, Sonar, sonar_next, sonar_node};
use crate::npcs::{Npcs, ScriptName};
use crate::{hud, paths, scripts};

/// The needle's, the inner pin's and the outer pin's images.
const NEEDLE: u32 = 101;
const PIN: u32 = 136;
const OUTER_PIN: u32 = 137;
/// The compass's centre on the screen of 1024 × 768, and the inner and the outer pins' ellipses'
/// half axes.
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

/// The marker 3's two pins: on the outer ellipse where `true`, on the inner one where `false`.
#[derive(Component)]
struct GroupPin(bool);

/// What a script's `GroupIsDead` asked of a group (its `Mind::group`) and found (`0x48C860`,
/// `scripts`).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GroupAsked {
    pub group: u32,
    pub dead: bool,
}

/// The marker 3 (`+0x7704`, `+0x7708`, `+0x7718`, `+0x771C`): the creature that `GroupIsDead`
/// found the nearest living member of its group, and the real time in seconds until which it
/// shows; a group found dead clears it.
#[derive(Resource, Default)]
struct Blinking(Option<(Entity, f64)>);

/// How long the marker 3 stays, in seconds of the real time, how far its member is sought, and
/// how long each of its two pins shows (`0x48CA63`–`0x48CA8B`, `0x6E3F70`).
const BLINKS_FOR: f64 = 60.0;
const MEMBER_WITHIN: f32 = 25_000.0;
const BLINK_MS: u128 = 300;

/// A sonar's ring, while it spreads.
#[derive(Component, Default)]
struct Ring(Option<f32>);

/// The needle and the two pins, hidden, and the sonars' rings on the bar.
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
    let pointer = |commands: &mut Commands, marker: Marker, index: u32| {
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
    };
    pointer(&mut commands, Marker::Main, NEEDLE);
    // The marker 3's pins after the needle, before the markers 1 and 2 (`0x6E3F70`).
    for (outer, index) in [(true, OUTER_PIN), (false, PIN)] {
        let size = images.size(index);
        let (image, node) = bar::piece(&images, &assets, index, CENTRE - size / 2.0, BAR.min);
        commands.entity(bar).with_child((
            GroupPin(outer),
            image,
            node,
            UiTransform::IDENTITY,
            Visibility::Hidden,
            ZIndex(2),
            Pickable::IGNORE,
        ));
    }
    pointer(&mut commands, Marker::Side, PIN);
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

/// The direction of a marker's target `to` from `from`, places on the ground (`x`, `z`), on
/// the screen's axes (`x` right, `y` down): their difference turned by 45° alone, the cells'
/// `x − y` across and `x + y` down, not squashed 2:1 as the isometric view draws the ground
/// (`0x4A5980`, #3244). The pins on their ellipses of 2:1 thus stand where the target lies on the
/// screen, and the needle and the overview's arrows take it as it is.
pub(crate) fn direction(from: Vec2, to: Vec2) -> Vec2 {
    let step = to - from;
    Vec2::new(step.x - step.y, step.x + step.y)
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
    objects: Query<'w, 's, (&'static crate::npcs::ObjectName, &'static GlobalTransform)>,
    npcs: Option<Res<'w, Npcs>>,
    dungeons: Option<Res<'w, crate::daynight::Dungeons>>,
}

impl Seek<'_, '_> {
    /// Where the creature or the script's object of the script name `name` stands on the ground
    /// (`x`, `z`), and whether it is in the world: its place there, else the cell where it lives
    /// away from it; the original looks the name up among the world's objects whatever they are
    /// (`0x4A5980`; the objects since #3105).
    fn creature(&self, name: &str) -> Option<(Vec2, bool)> {
        if let Some((_, at)) = self
            .named
            .iter()
            .find(|(script, _)| script.0.eq_ignore_ascii_case(name))
        {
            return Some((at.translation().xz(), true));
        }
        if let Some((_, at)) = self
            .objects
            .iter()
            .find(|(object, _)| object.0.eq_ignore_ascii_case(name))
        {
            return Some((at.translation().xz(), true));
        }
        let npcs = self.npcs.as_deref()?;
        let (x, y) = npcs.away(name).or_else(|| npcs.object_away(name))?;
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

    /// Where a marker points at the place `target` from the hero standing at `hero` (`x`, `z`),
    /// the marker of the quest `quest` where it is a quest's: the place, or across a dungeon's
    /// mouth between them the corner of the way's cell, where the original's resolve puts a
    /// way's point (`0x4A6079`; `scripts::Live::toward`, #3419). Every marker goes through it,
    /// the needle, the three pins and the overview's arrows (`0x6E3FE7`, `0x6CFB49`).
    pub(crate) fn aim_at(
        &self,
        live: Option<&scripts::Live>,
        (hero, target): (Vec2, Vec2),
        quest: Option<u32>,
    ) -> Vec2 {
        let (Some(live), Some(dungeons)) = (live, self.dungeons.as_deref()) else {
            return target;
        };
        let cell = paths::cell_of(target.x, target.y);
        let way = live.toward((paths::cell_of(hero.x, hero.y), cell), quest, dungeons);
        if way == cell {
            target
        } else {
            let corner = crate::iso::ground(way.0 as f32, way.1 as f32);
            Vec2::new(corner.x, corner.z)
        }
    }

    /// Where the quest marker `marker` points from the hero standing at `hero` ([`Self::aim_at`]).
    pub(crate) fn aim(&self, live: &scripts::Live, hero: Vec2, marker: Marker) -> Option<Vec2> {
        let (quest, compass) = live.quests().marked(marker)?;
        let target = self.target_of(compass)?;
        Some(self.aim_at(Some(live), (hero, target), Some(quest)))
    }

    /// Where the world map's icons and the quest book's picture show the quest `quest`'s target
    /// `compass`: its place, or in a hidden sector the centre of the way into it, the entry's
    /// cell that they read (`scripts::Live::shown_at`, #3419).
    pub(crate) fn shown_at(
        &self,
        live: &scripts::Live,
        quest: u32,
        compass: &Compass,
    ) -> Option<Vec2> {
        let target = self.target_of(compass)?;
        let Some(dungeons) = self.dungeons.as_deref() else {
            return Some(target);
        };
        let cell = paths::cell_of(target.x, target.y);
        let way = live.shown_at(cell, quest, dungeons);
        Some(if way == cell {
            target
        } else {
            paths::centre(way)
        })
    }
}

/// Where a marker of the compass points, as the state tells it (#2403): its quest, the target's
/// cell (a creature's where it stands now, none for one killed or not yet made), the
/// creature's name in the scripts where the target is one, whether it lives away from the
/// world (#2406), and the cell of the way across a dungeon's mouth that the marker points at
/// instead of the target (#3419).
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Aim {
    pub quest: u32,
    pub cell: Option<paths::Cell>,
    pub name: Option<String>,
    pub away: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub way: Option<paths::Cell>,
}

impl Aim {
    /// The marker of the quest `quest` toward `compass`, the hero's cell `hero` and the scripts
    /// `live` giving its way ([`Seek::aim`]).
    fn of(quest: u32, compass: &Compass, seek: &Seek, way: Option<(&scripts::Live, Vec2)>) -> Self {
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
        let way = way.and_then(|(live, hero)| {
            let target = seek.target_of(compass)?;
            let aimed = seek.aim_at(Some(live), (hero, target), Some(quest));
            (aimed != target).then(|| paths::cell_of(aimed.x, aimed.y))
        });
        Self {
            quest,
            cell,
            name,
            away,
            way,
        }
    }

    /// The state's words for it: `quest N at X,Y`, the creature's name after the cell, and
    /// `via X,Y` for the way the marker points at instead.
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
        if let Some((x, y)) = self.way {
            line.push_str(&format!(" via {x},{y}"));
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

/// Keeps [`Targets`] to the markers' quests, their targets' places and their ways.
fn aim(
    scripts: Option<NonSend<scripts::Live>>,
    heroes: Query<&GlobalTransform, With<Body>>,
    seek: Seek,
    mut targets: ResMut<Targets>,
) {
    let hero = heroes.iter().next().map(|hero| hero.translation().xz());
    let of = |marker| {
        let live = scripts.as_deref()?;
        let (quest, compass) = live.quests().marked(marker)?;
        Some(Aim::of(
            quest,
            compass,
            &seek,
            hero.map(|hero| (live, hero)),
        ))
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
    images: Res<UiImages>,
    mut pointers: Query<(&Pointer, &mut Node, &mut UiTransform, &mut Visibility)>,
) {
    let hero = heroes.iter().next().map(GlobalTransform::translation);
    for (pointer, mut node, mut transform, mut visibility) in &mut pointers {
        let shown = scripts.as_ref().zip(hero).and_then(|(live, hero)| {
            let target = seek.aim(live, hero.xz(), pointer.0)?;
            Some(angle(direction(hero.xz(), target)))
        });
        let Some(phi) = shown else {
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
        }
    }
}

/// Keeps the marker 3 by what `GroupIsDead` found (`0x48C860`): of a group alive, its member
/// nearest the hero within 25,000 for 60 s of the real time from now; a group dead clears it.
fn group_asked(
    mut asked: MessageReader<GroupAsked>,
    time: Res<Time<Real>>,
    mut blinking: ResMut<Blinking>,
    heroes: Query<&GlobalTransform, With<Body>>,
    members: Query<
        (
            Entity,
            &GlobalTransform,
            &crate::ai::Mind,
            &crate::fight::Health,
        ),
        Without<crate::npcs::Dies>,
    >,
) {
    let hero = heroes.iter().next().map(GlobalTransform::translation);
    for asked in asked.read() {
        if asked.dead {
            blinking.0 = None;
            continue;
        }
        let Some(hero) = hero else {
            continue;
        };
        let nearest = members
            .iter()
            .filter(|(_, _, mind, health)| {
                mind.group == Some(asked.group)
                    && mind.flags & crate::followers::RAISED == 0
                    && health.current > 0
            })
            .map(|(entity, at, ..)| (entity, (at.translation() - hero).xz().length()))
            .filter(|&(_, d)| d <= MEMBER_WITHIN)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((member, _)) = nearest {
            blinking.0 = Some((member, time.elapsed_secs_f64() + BLINKS_FOR));
        }
    }
}

/// Whether the marker 3 shows its outer pin at the real time of `ms` milliseconds: an even
/// `⌊ms/300⌋`, the inner one at an odd (`0x6E3F70`).
fn outer_phase(ms: u128) -> bool {
    (ms / BLINK_MS).is_multiple_of(2)
}

/// The marker 3's pins: while it stands, toward its creature, the outer pin on its ellipse at an
/// even 300 ms of the real time and the inner one at an odd, each turned by its angle; gone at
/// its 60 s or with its creature.
fn point_group(
    time: Res<Time<Real>>,
    mut blinking: ResMut<Blinking>,
    heroes: Query<&GlobalTransform, With<Body>>,
    places: Query<&GlobalTransform, Without<crate::npcs::Dies>>,
    (live, seek): (Option<NonSend<scripts::Live>>, Seek),
    images: Res<UiImages>,
    mut pins: Query<(&GroupPin, &mut Node, &mut UiTransform, &mut Visibility)>,
) {
    let now = time.elapsed_secs_f64();
    if blinking.0.is_some_and(|(_, until)| now >= until) {
        blinking.0 = None;
    }
    let hero = heroes.iter().next().map(GlobalTransform::translation);
    let phi = blinking
        .0
        .and_then(|(member, _)| places.get(member).ok())
        .zip(hero)
        .map(|(at, hero)| {
            let target = seek.aim_at(live.as_deref(), (hero.xz(), at.translation().xz()), None);
            angle(direction(hero.xz(), target))
        });
    let outer = outer_phase(time.elapsed().as_millis());
    for (pin, mut node, mut transform, mut visibility) in &mut pins {
        let Some(phi) = phi.filter(|_| pin.0 == outer) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        visibility.set_if_neq(Visibility::Inherited);
        transform.rotation = Rot2::radians(phi);
        let (axes, index) = if pin.0 {
            (OUTER, OUTER_PIN)
        } else {
            (INNER, PIN)
        };
        let at = on_ellipse(phi, axes) - images.size(index) / 2.0 - BAR.min;
        node.left = Val::Px(at.x);
        node.top = Val::Px(at.y);
    }
}

/// Turns the outer pin on its ellipse of 58 × 29 toward the world map's pin, the marker 2
/// (`0x4A63E0`), hidden while no pin is placed.
fn point_outer(
    map: Res<crate::world_map::Map>,
    heroes: Query<&GlobalTransform, With<Body>>,
    (live, seek): (Option<NonSend<scripts::Live>>, Seek),
    images: Res<UiImages>,
    mut pins: Query<(&mut Node, &mut UiTransform, &mut Visibility), With<OuterPin>>,
) {
    let hero = heroes.iter().next().map(GlobalTransform::translation);
    let phi = map.pin_target().zip(hero).map(|(target, hero)| {
        let target = seek.aim_at(live.as_deref(), (hero.xz(), target), None);
        angle(direction(hero.xz(), target))
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
            .init_resource::<Blinking>()
            .add_message::<GroupAsked>()
            .add_systems(
                OnEnter(AppState::InGame),
                (spawn.after(bar::spawn), |mut blinking: ResMut<Blinking>| {
                    blinking.0 = None;
                }),
            )
            .add_systems(
                Update,
                (aim, point, point_outer, group_asked, point_group, sonar)
                    .after(bar::fill)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_group_pin_blinks_outer_then_inner_every_300_ms() {
        let phases: Vec<bool> = [0, 299, 300, 599, 600, 900].map(outer_phase).to_vec();
        assert_eq!(phases, [true, true, false, false, true, false]);
    }

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
    fn a_markers_direction_is_turned_by_45_degrees_alone() {
        // The cells' x runs right and down the screen, their y left and down, at 45° each.
        let step = |x: f32, y: f32| {
            let ground = |x: f32, y: f32| crate::iso::ground(x, y).xz();
            direction(ground(0.0, 0.0), ground(x, y))
        };
        let x = step(200.0, 0.0);
        let y = step(0.0, 200.0);
        assert!(x.x > 0.0 && (x.x - x.y).abs() < 1e-2, "{x}");
        assert!(y.x < 0.0 && (y.x + y.y).abs() < 1e-2, "{y}");
        // The Seraphim's start's novice, two cells across and seven down: the original's
        // (−188, 341) of `0x4A5980`, about 29° left of straight down.
        let novice = step(2.0, 7.0);
        assert!((novice.x / novice.y + 5.0 / 9.0).abs() < 1e-3, "{novice}");
        assert!((angle(novice).to_degrees() + 151.0).abs() < 0.5);
    }
}
