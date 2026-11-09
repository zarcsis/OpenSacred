//! The scripts' cinema's look (#2393; docs/re/netscript.md, "The anim mode and the creatures'
//! queues"): the interface manager takes each `SetCinemaMode` and each `SetPlayMode` as a flag
//! (`cUI_Manager::receiveEvent`, `0x757840`'s case 5), and while it holds it draws only the Esc
//! menu, the tooltips (the dialogues' windows among them) and two black bars that slide in over
//! the top and the bottom of the original's 768-pixel-high screen (`cUI_Manager_render`,
//! `0x758ED0`). The bar, the portraits and every other window of the interface are hidden
//! meanwhile ([`Interface`]), and the player's keys and clicks go to none of them (`Control`
//! held back). After a mode of a second or more the bars slide out the same way, and the
//! interface comes back once they are gone. Meanwhile the keys but the game's own tools' go to
//! nothing, and the windows' toggles open none (the manager's events go to the Esc menu and
//! the tooltips alone, `0x757AB4`; seen: I, L, M and Esc opened nothing).

use bevy::prelude::*;
use bevy::time::Real;

use crate::frame::{Hold, HoldsControl, Playing};
use crate::game::AppState;
use crate::scripts::CinemaFlag;

/// A root of the interface that the cinema hides: the bar, the portraits' column, the foe's
/// panel and the game's windows, the original manager's windows but its Esc menu and its
/// tooltips (`+0x124`, `0x758DC0`), which hold the dialogues.
#[derive(Component)]
pub struct Interface;

/// How fast the bars slide, in the original's pixels a second (`0x758FD9`).
const SLIDE: f32 = 30.0;
/// How high each bar stands at the most, in the original's pixels.
const HIGH: f32 = 64.0;
/// The least mode after which the bars slide out rather than go at once, in seconds.
const SLIDES_OUT: f32 = 1.0;
/// The bars' layer: over the windows, the tooltips and the labels, under the Esc menu's 20.
const LAYER: i32 = 19;

/// The cinema's look as the interface manager keeps it (its `+0x08` bits `0x10` and `0x20`, and
/// the real clock it noted at `+0x78`).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct Look {
    /// Whether the look shows (`0x10`).
    shown: bool,
    /// Whether its bars slide out (`0x20`).
    ending: bool,
    /// The real clock in seconds as last noted.
    since: f32,
}

impl Look {
    /// The look after a flag of the scripts at the real clock `now` (`0x757840`'s case 5): on,
    /// it shows from now, the bars' sliding out kept where it was set; off, after a mode of a
    /// second or more since the clock was last noted, the bars slide out from now, else the look
    /// goes at once. So a `SetPlayMode` with no cinema before it makes the next cinema's bars
    /// stand whole at once and slide out, the interface back before the mode ends (seen).
    fn flag(&mut self, on: bool, now: f32) {
        if on {
            self.shown = true;
            self.since = now;
        } else if (self.since - now).abs() >= SLIDES_OUT {
            self.ending = true;
            self.since = now;
        } else {
            self.shown = false;
        }
    }

    /// The bars' height at the real clock `now`, in the original's pixels, none where the look
    /// does not show (`0x758FC5` to `0x75902D`): 30 a second since the clock was noted, at most
    /// 64, from 64 down under `0x20`, the look gone once that is 0 or less.
    fn height(&mut self, now: f32) -> Option<f32> {
        if !self.shown {
            return None;
        }
        let high = ((now - self.since) * SLIDE).min(HIGH);
        if !self.ending {
            return Some(high);
        }
        let high = HIGH - high;
        if high <= 0.0 {
            self.shown = false;
            self.ending = false;
            return None;
        }
        Some(high)
    }

    /// Whether the look shows.
    pub fn shown(&self) -> bool {
        self.shown
    }
}

/// One of the two bars, the top one or the bottom one.
#[derive(Component)]
struct Bar;

/// A root of the interface that the look hides, and its visibility before.
#[derive(Component)]
struct Veiled(Visibility);

/// A new game's look, none, and its two bars.
fn begin(mut commands: Commands) {
    commands.insert_resource(Look::default());
    for top in [true, false] {
        commands.spawn((
            Bar,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                top: if top { Val::Px(0.0) } else { Val::Auto },
                bottom: if top { Val::Auto } else { Val::Px(0.0) },
                height: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(Color::BLACK),
            GlobalZIndex(LAYER),
            Pickable::IGNORE,
            Visibility::Hidden,
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// Follows the scripts' flags, sizes the bars and hides the interface while the look shows,
/// bringing it back as it was once the look goes.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn show(
    mut commands: Commands,
    (mut flags, real, look): (
        MessageReader<CinemaFlag>,
        Res<Time<Real>>,
        Option<ResMut<Look>>,
    ),
    mut bars: Query<(&mut Node, &mut Visibility), With<Bar>>,
    mut shown: Query<(Entity, &mut Visibility), (With<Interface>, Without<Veiled>, Without<Bar>)>,
    mut veiled: Query<(Entity, &mut Visibility, &Veiled), Without<Bar>>,
) {
    let Some(mut look) = look else {
        flags.clear();
        return;
    };
    let now = real.elapsed_secs();
    for flag in flags.read() {
        look.flag(flag.0, now);
    }
    let height = look.height(now);
    for (mut node, mut visibility) in &mut bars {
        node.height = Val::Px(height.unwrap_or(0.0));
        visibility.set_if_neq(if height.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    if look.shown {
        for (entity, mut visibility) in &mut shown {
            commands.entity(entity).insert(Veiled(*visibility));
            *visibility = Visibility::Hidden;
        }
        return;
    }
    for (entity, mut visibility, before) in &mut veiled {
        *visibility = before.0;
        commands.entity(entity).remove::<Veiled>();
    }
}

/// Whether the look leaves the player's keys and clicks to the interface and the world: the
/// condition of the windows' toggles, which run before `Control` is held back.
pub fn free(look: Option<Res<Look>>) -> bool {
    !look.is_some_and(|look| look.shown())
}

/// Takes the keys pressed while the look shows, but the game's own tools', from the interface and
/// the world.
fn take_keys(look: Option<Res<Look>>, mut keys: crate::controls::KeysMut) {
    if !free(look) {
        keys.take_all_but(crate::controls::Section::Tools);
    }
}

/// The cinema's look.
pub struct CinemaPlugin;

impl Plugin for CinemaPlugin {
    fn build(&self, app: &mut App) {
        app.hold_control(free)
            .add_systems(OnEnter(AppState::InGame), begin)
            .add_systems(OnExit(AppState::InGame), |mut commands: Commands| {
                commands.remove_resource::<Look>();
            })
            .add_systems(PreUpdate, take_keys.in_set(Hold))
            .add_systems(Update, show.in_set(Playing));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bars_slide_in_and_after_a_second_or_more_slide_out() {
        let mut look = Look::default();
        assert_eq!(look.height(100.0), None);
        look.flag(true, 100.0);
        // 30 pixels a second, up to 64 (seen: 5, 25, 45 and 64 at frames 0.68 s apart).
        assert_eq!(look.height(100.5), Some(15.0));
        assert_eq!(look.height(102.0), Some(60.0));
        assert_eq!(look.height(110.0), Some(64.0));
        // Off after 9 s: they slide out from 64, then the look goes.
        look.flag(false, 110.0);
        assert!(look.shown());
        assert_eq!(look.height(110.5), Some(49.0));
        assert_eq!(look.height(112.2), None);
        assert!(!look.shown());
        // A mode shorter than a second goes at once.
        look.flag(true, 200.0);
        look.flag(false, 200.5);
        assert!(!look.shown());
        assert_eq!(look.height(200.6), None);
    }

    #[test]
    fn a_play_mode_with_no_cinema_before_makes_the_next_bars_slide_out_at_once() {
        // Seen: `hq_starten`'s `SetPlayMode`, then `hq15_close_gate`'s cinema: its bars 59, 38
        // and 17 pixels high at frames 0.7 s apart, the interface back while the mode held.
        let mut look = Look::default();
        look.flag(false, 50.0);
        assert!(!look.shown());
        look.flag(true, 60.0);
        assert_eq!(look.height(60.0), Some(64.0));
        assert_eq!(look.height(61.0), Some(34.0));
        assert_eq!(look.height(62.2), None);
        assert!(!look.shown());
    }

    #[test]
    fn the_look_hides_the_interface_and_brings_it_back_as_it_was() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<CinemaFlag>()
            .insert_resource(Look::default())
            .add_systems(Update, show);
        let bar = app
            .world_mut()
            .spawn((Interface, Visibility::Inherited))
            .id();
        let closed = app.world_mut().spawn((Interface, Visibility::Hidden)).id();
        let talk = app.world_mut().spawn(Visibility::Inherited).id();
        let visibility = |app: &App, entity| *app.world().get::<Visibility>(entity).unwrap();
        app.world_mut().write_message(CinemaFlag(true));
        app.update();
        assert_eq!(visibility(&app, bar), Visibility::Hidden);
        assert_eq!(visibility(&app, talk), Visibility::Inherited);
        assert!(app.world().resource::<Look>().shown());
        // A mode shorter than a second: the look goes at once, each root as it was.
        app.world_mut().write_message(CinemaFlag(false));
        app.update();
        app.update();
        assert_eq!(visibility(&app, bar), Visibility::Inherited);
        assert_eq!(visibility(&app, closed), Visibility::Hidden);
        assert!(app.world().get::<Veiled>(bar).is_none());
    }
}
