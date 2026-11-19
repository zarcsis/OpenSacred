//! The scripts' pictures (#2891; docs/re/netscript.md, `InfoPlayer`): `InfoPlayer`'s mode 10
//! names one of `Sacred.exe`'s three bitmaps by its file's name, `witz1.bmp` to `witz3.bmp`, which
//! `0x4AE350` loads (`LoadImageA` of the resources 125, 124 and 126) and appends to the engine's
//! list; while the list holds one, the render thread copies its first over the frame, 1024 × 768
//! from the top left, and a press and release of the left button takes it off the list
//! (`0x60E76A`: `GetAsyncKeyState`, its latch `0xAD6F04`). Another name shows nothing. The
//! converter writes the three as `ui/screens/witz1.png` to `witz3.png` (`opensacred-convert ui`).
//! The game shows the picture as the original's screen over the window, black about it, and keeps
//! the clicks from the world while it shows, which the original does not read.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::frame::HoldsControl;
use crate::game::AppState;
use crate::scripts;

/// `InfoPlayer`'s mode of the pictures.
const MODE: i32 = 10;
/// The pictures' names in the scripts and their files among the assets, as `0x4AE350` compares
/// the names, case aside.
const PICTURES: [(&str, &str); 3] = [
    ("witz1.bmp", "ui/screens/witz1.ktx2"),
    ("witz2.bmp", "ui/screens/witz2.ktx2"),
    ("witz3.bmp", "ui/screens/witz3.ktx2"),
];
/// The pictures' layer: over the interface, its windows and the fades, under the movies and the
/// tools.
const LAYER: i32 = i32::MAX - 3;

/// The pictures to show, the first showing, and whether the left button was down while it showed
/// (`0xAD6F04`).
#[derive(Resource, Default, Debug, PartialEq)]
pub struct Pictures {
    queue: VecDeque<&'static str>,
    pressed: bool,
}

impl Pictures {
    /// The file of the picture that `name` names, case aside; none for another name.
    fn file(name: &str) -> Option<&'static str> {
        PICTURES
            .iter()
            .find(|(known, _)| known.eq_ignore_ascii_case(name))
            .map(|&(_, file)| file)
    }

    /// The left button in this frame: down, it is latched; up after a latch, the first picture
    /// goes. Whether one went.
    fn button(&mut self, down: bool) -> bool {
        if self.queue.is_empty() {
            return false;
        }
        if down {
            self.pressed = true;
            return false;
        }
        if !std::mem::take(&mut self.pressed) {
            return false;
        }
        self.queue.pop_front();
        true
    }
}

/// The picture that shows.
#[derive(Component)]
struct Shown(&'static str);

/// Takes the pictures' mode from the scripts' `InfoPlayer`s before their texts are shown, each
/// onto the list in order.
fn take(mut infos: ResMut<scripts::Infos>, mut pictures: ResMut<Pictures>) {
    infos.0.retain(|(mode, name)| {
        if *mode != MODE {
            return true;
        }
        match Pictures::file(name) {
            Some(file) => pictures.queue.push_back(file),
            None => debug!("pictures: no picture {name}"),
        }
        false
    });
}

/// Takes the first picture off by the left button's press and release.
fn click(mut pictures: ResMut<Pictures>, buttons: Res<ButtonInput<MouseButton>>) {
    if pictures.button(buttons.pressed(MouseButton::Left)) {
        info!("pictures: a picture closed by the click");
    }
}

/// Shows the first picture, the original's screen over the window and black about it, and takes
/// it away as it goes; a picture the assets lack goes at once.
fn show(
    mut commands: Commands,
    mut pictures: ResMut<Pictures>,
    assets: Res<AssetServer>,
    shown: Query<(Entity, &Shown)>,
) {
    let first = loop {
        match pictures.queue.front() {
            Some(file) if !crate::files::exists(crate::files::path(file)) => {
                warn!("pictures: no {file} among the assets");
                pictures.queue.pop_front();
            }
            other => break other.copied(),
        }
    };
    let mut current = None;
    for (entity, picture) in &shown {
        if Some(picture.0) == first {
            current = Some(entity);
        } else {
            commands.entity(entity).despawn();
        }
    }
    let Some(file) = first.filter(|_| current.is_none()) else {
        return;
    };
    info!("pictures: {file} shows");
    commands
        .spawn((
            Shown(file),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::BLACK),
            GlobalZIndex(LAYER),
            DespawnOnExit(AppState::InGame),
        ))
        // The original's screen, as high as the window (`hud`).
        .with_child((
            ImageNode::new(crate::hud::image(&assets, file)),
            Node {
                width: Val::Px(1024.0),
                height: Val::Px(768.0),
                ..default()
            },
        ));
}

/// Forgets the pictures as the game ends.
fn clear(mut pictures: ResMut<Pictures>) {
    *pictures = Pictures::default();
}

/// The scripts' pictures.
pub struct PicturesPlugin;

impl Plugin for PicturesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pictures>()
            .hold_clicks(|pictures: Res<Pictures>| pictures.queue.is_empty())
            .add_systems(
                Update,
                (
                    take.after(crate::talk::popups).before(crate::talk::notices),
                    click,
                    show,
                )
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(OnExit(AppState::InGame), clear);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scripts_names_choose_the_pictures_and_the_other_infos_stay() {
        let mut app = App::new();
        app.init_resource::<scripts::Infos>()
            .init_resource::<Pictures>()
            .add_systems(Update, take);
        app.world_mut().resource_mut::<scripts::Infos>().0 = vec![
            (MODE, "WITZ2.bmp".into()),
            (0, "res:17643".into()),
            (MODE, "witz1.bmp".into()),
            (MODE, "witz4.bmp".into()),
            (MODE, "witz3.bmp".into()),
        ];
        app.update();
        assert_eq!(
            app.world().resource::<scripts::Infos>().0,
            [(0, "res:17643".to_owned())]
        );
        assert_eq!(
            app.world().resource::<Pictures>().queue,
            [
                "ui/screens/witz2.ktx2",
                "ui/screens/witz1.ktx2",
                "ui/screens/witz3.ktx2"
            ]
        );
    }

    #[test]
    fn a_press_and_release_of_the_left_button_takes_the_first_picture_off() {
        let mut pictures = Pictures {
            queue: VecDeque::from(["a", "b"]),
            pressed: false,
        };
        // Up with no press before, and down, take nothing off.
        assert!(!pictures.button(false));
        assert!(!pictures.button(true));
        assert!(!pictures.button(true));
        assert_eq!(pictures.queue, ["a", "b"]);
        // Up after it, the first goes and the latch clears.
        assert!(pictures.button(false));
        assert_eq!(pictures.queue, ["b"]);
        assert!(!pictures.pressed);
        assert!(!pictures.button(false));
        assert!(!pictures.button(true));
        assert!(pictures.button(false));
        assert!(pictures.queue.is_empty());
        // With none to show, a press latches nothing.
        assert!(!pictures.button(true));
        assert!(!pictures.pressed);
    }
}
