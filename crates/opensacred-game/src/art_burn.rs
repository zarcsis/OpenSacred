//! The burn of the bar's art slots (#1464; docs/re/interface.md, "The burn"): as the hero's form
//! changes, the Vampiress's day and night (`0x557040`, which posts the taskbar's message `0x35`
//! for the local hero as her arts change with her form), every visible art slot shows the 16
//! frames of `FX_BUTTONBURN` at 16 a second over its icon and under its ring, the frame f the part
//! (f mod 4, f div 4) · 64 of the 256 × 256 texture, and then none.

use std::time::Duration;

use bevy::prelude::*;

use crate::bar::{self, BAR, BarRoot, UiImages};
use crate::forms::Forms;
use crate::game::AppState;
use crate::hud;

/// The burn's texture.
pub(crate) const BURN: &str = "fx_buttonburn";
/// Its frames, their rate a second, and a frame's side in its 4 × 4 grid.
const FRAMES: u8 = 16;
const RATE: u128 = 16;
const SIDE: f32 = 64.0;
/// Its layer: over the slot's icon, under its ring.
const BURN_Z: i32 = bar::RING_Z - 1;

/// A slot's burn, while it plays.
#[derive(Component)]
struct Burning;

/// The burn's start in real time, while it plays, and the hero's form as last seen.
#[derive(Resource, Default)]
struct Burn {
    since: Option<Duration>,
    night: Option<bool>,
}

/// The frame `f`'s part of the texture.
fn frame_rect(frame: u8) -> Rect {
    let at = Vec2::new(f32::from(frame % 4), f32::from(frame / 4)) * SIDE;
    Rect::from_corners(at, at + Vec2::splat(SIDE))
}

/// The burn's frame `played` after its start, none once its 16 have played.
fn frame_at(played: Duration) -> Option<u8> {
    u8::try_from(played.as_millis() * RATE / 1000)
        .ok()
        .filter(|&frame| frame < FRAMES)
}

/// A change of the hero's form starts the burn.
fn start(time: Res<Time<Real>>, forms: Option<Res<Forms>>, mut burn: ResMut<Burn>) {
    let Some(forms) = forms else {
        return;
    };
    let night = forms.by_night();
    if burn.night.is_some_and(|before| before != night) {
        burn.since = Some(time.elapsed());
    }
    burn.night = Some(night);
}

/// Shows the burn's frame over each visible art slot, and takes it away once it has played.
fn play(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut burn: ResMut<Burn>,
    experience: Res<crate::fight::Experience>,
    (images, assets): (Res<UiImages>, Res<AssetServer>),
    bars: Query<Entity, With<BarRoot>>,
    mut burning: Query<(Entity, &mut ImageNode), With<Burning>>,
) {
    let frame = burn
        .since
        .and_then(|since| frame_at(time.elapsed().saturating_sub(since)));
    let Some(frame) = frame else {
        burn.since = None;
        for (entity, _) in &burning {
            commands.entity(entity).despawn();
        }
        return;
    };
    let slots = bar::slots(experience.0.level);
    if burning.iter().count() != slots {
        for (entity, _) in &burning {
            commands.entity(entity).despawn();
        }
        let (Ok(root), Some(file)) = (bars.single(), images.file(BURN)) else {
            return;
        };
        let texture = hud::image(&assets, file.to_owned());
        for i in 0..slots {
            let at = bar::art_slot(i) - BAR.min;
            commands.entity(root).with_child((
                Burning,
                ImageNode {
                    rect: Some(frame_rect(frame)),
                    ..ImageNode::new(texture.clone())
                },
                bar::placed(at, Vec2::splat(SIDE)),
                ZIndex(BURN_Z),
                Pickable::IGNORE,
            ));
        }
        return;
    }
    for (_, mut image) in &mut burning {
        image.rect = Some(frame_rect(frame));
    }
}

/// The art slots' burn.
pub struct ArtBurnPlugin;

impl Plugin for ArtBurnPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Burn>().add_systems(
            Update,
            (start, play)
                .chain()
                .after(bar::fill)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_burn_plays_its_sixteen_frames_from_its_grid() {
        let ms = Duration::from_millis;
        assert_eq!(frame_at(ms(0)), Some(0));
        assert_eq!(frame_at(ms(70)), Some(1));
        assert_eq!(frame_at(ms(999)), Some(15));
        assert_eq!(frame_at(ms(1000)), None);
        assert_eq!(frame_rect(0), Rect::new(0.0, 0.0, 64.0, 64.0));
        assert_eq!(frame_rect(5), Rect::new(64.0, 64.0, 128.0, 128.0));
        assert_eq!(frame_rect(15), Rect::new(192.0, 192.0, 256.0, 256.0));
    }
}
