//! The taskbar's points "+" (#1522; docs/re/interface.md, docs/re/character.md): the image 81 at
//! (898, 10) of the original's screen, beside the hero's portrait, shown and glowing as the hero
//! rises a level (the taskbar's message 8, `0x57EFCE`), its glow stopped by a point spent in the
//! statistics window (the message 7 with 2) and hidden once no points of any kind remain (7 with
//! 1); a click on it opens the inventory's windows. Its glow is the portrait's ring's
//! (`0x732220`) at a = (sin 6T + 1) · 0.3.

use bevy::prelude::*;
use bevy::ui_render::prelude::MaterialNode;

use crate::bar::{self, ColumnRoot, UiImages};
use crate::fight::Experience;
use crate::game::AppState;
use crate::hud::Pointer;
use crate::inventory::Shown;
use crate::low_health::{GLOW_REACH, Glow};
use crate::sfx;

/// The "+"'s image and its corner on the original's screen (`+0x18C`), left of the column at
/// x 932.
const PLUS: (u32, Vec2) = (81, Vec2::new(898.0, 10.0));
/// The glow's pulse and its share: a = (sin 6T + 1) · 0.3, T the real time in seconds, each
/// copy's alpha ⌊12.75 a⌋/255.
const GLOW_PULSE: f32 = 6.0;
const GLOW_SHARE: f32 = 0.3;
const GLOW_BYTE: f32 = 12.75;

/// Whether the "+" shows and whether it glows (its flag `0x400000`).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Plus {
    pub shown: bool,
    pub glowing: bool,
}

/// The "+"'s node and its glow's.
#[derive(Component)]
struct PlusNode;
#[derive(Component)]
struct PlusGlow;

/// The "+"'s rectangle on the original's screen.
pub(crate) fn plus_rect(images: &UiImages) -> Rect {
    Rect::from_corners(PLUS.1, PLUS.1 + images.size(PLUS.0))
}

/// The "+" in the column, hidden, with its glow over it.
fn spawn(
    mut commands: Commands,
    images: Res<UiImages>,
    assets: Res<AssetServer>,
    mut glows: ResMut<Assets<Glow>>,
    columns: Query<Entity, With<ColumnRoot>>,
    mut plus: ResMut<Plus>,
) {
    *plus = Plus::default();
    let Ok(column) = columns.single() else {
        return;
    };
    let (image, node) = bar::piece(&images, &assets, PLUS.0, PLUS.1, bar::COLUMN.min);
    let size = images.size(PLUS.0);
    let glow = glows.add(Glow::empty());
    commands.entity(column).with_children(|column| {
        column
            .spawn((PlusNode, image, node, Visibility::Hidden, Pickable::IGNORE))
            .with_child((
                PlusGlow,
                MaterialNode(glow),
                bar::placed(
                    Vec2::splat(-GLOW_REACH),
                    size + Vec2::splat(2.0 * GLOW_REACH),
                ),
                Visibility::Hidden,
                Pickable::IGNORE,
            ));
    });
}

/// The "+" shown and glowing as the hero rises a level, hidden once no points remain.
fn follow(experience: Res<Experience>, mut plus: ResMut<Plus>, mut last: Local<Option<u32>>) {
    let level = experience.level;
    if last.is_some_and(|last| level > last) {
        *plus = Plus {
            shown: true,
            glowing: true,
        };
    }
    *last = Some(level);
    if experience.attribute_points == 0
        && experience.skill_points == 0
        && experience.to_learn() == 0
    {
        *plus = Plus::default();
    }
}

/// The alpha of each of the glow's copies at the real time `seconds`.
fn glow_alpha(seconds: f32) -> f32 {
    let pulse = ((GLOW_PULSE * seconds).sin() + 1.0) * GLOW_SHARE;
    (GLOW_BYTE * pulse).floor() / 255.0
}

/// The "+" and its glow as [`Plus`] says.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn draw(
    time: Res<Time<Real>>,
    plus: Res<Plus>,
    mut nodes: Query<(&ImageNode, &mut Visibility), (With<PlusNode>, Without<PlusGlow>)>,
    mut glowing: Query<(&MaterialNode<Glow>, &mut Visibility), With<PlusGlow>>,
    mut glows: ResMut<Assets<Glow>>,
) {
    let shown = if plus.shown {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let mut picture = None;
    for (image, mut visibility) in &mut nodes {
        visibility.set_if_neq(shown);
        picture = Some(image);
    }
    let alpha = glow_alpha(time.elapsed_secs());
    for (material, mut visibility) in &mut glowing {
        let on = plus.shown && plus.glowing && alpha > 0.0;
        visibility.set_if_neq(if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        let (Some(picture), Some(mut glow)) = (picture, glows.get_mut(&material.0)) else {
            continue;
        };
        if let Some(part) = picture.rect {
            glow.of(&picture.image, part, alpha);
        }
    }
}

/// A left click on the shown "+" opens the inventory's windows, with their sound.
fn click(
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    images: Res<UiImages>,
    plus: Res<Plus>,
    mut shown: ResMut<Shown>,
    mut sounds: MessageWriter<sfx::Play>,
) {
    if !plus.shown || !buttons.just_pressed(MouseButton::Left) || shown.0 {
        return;
    }
    let Some(at) = bar::column_point(&pointer, &ui_scale, &windows) else {
        return;
    };
    if !plus_rect(&images).contains(at) {
        return;
    }
    shown.0 = true;
    sounds.write(sfx::Play {
        sound: sfx::Of::Id(crate::inventory::OPENS),
        at: None,
        rate: (0.5, sfx::Roll::None),
    });
}

/// The taskbar's points "+".
pub struct PointsPlugin;

impl Plugin for PointsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Plus>()
            .add_systems(OnEnter(AppState::InGame), spawn.after(bar::spawn))
            .add_systems(
                Update,
                (follow, click, draw)
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_glow_pulses_up_to_a_third_of_its_byte() {
        // At its brightest a = 0.6: ⌊7.65⌋ = 7.
        let brightest = std::f32::consts::FRAC_PI_2 / GLOW_PULSE;
        assert!((glow_alpha(brightest) - 7.0 / 255.0).abs() < 1e-6);
        assert!(glow_alpha(-brightest).abs() < 1e-6);
    }

    #[test]
    fn the_plus_shows_at_a_level_up_and_hides_with_the_points() {
        let mut app = App::new();
        app.init_resource::<Experience>()
            .init_resource::<Plus>()
            .add_systems(Update, follow);
        app.update();
        assert_eq!(*app.world().resource::<Plus>(), Plus::default());
        let rules = opensacred_rules::fight::Rules::default();
        app.world_mut()
            .resource_mut::<Experience>()
            .0
            .add(300, &rules);
        app.update();
        assert_eq!(
            *app.world().resource::<Plus>(),
            Plus {
                shown: true,
                glowing: true
            }
        );
        let mut experience = app.world_mut().resource_mut::<Experience>();
        experience.attribute_points = 0;
        experience.skill_points = 0;
        app.update();
        assert_eq!(*app.world().resource::<Plus>(), Plus::default());
    }
}
