//! A player's hero knocked down by a heavy blow (#2311; docs/re/fight.md, "A knock-down"): the
//! strike's resolution marks it ([`fight::Downed`], `0x56E366`–`0x56E8CC`); here it falls by its
//! slot 228 at the rate 1.0, its near hostiles' strikes stopping (`0x54E6D0` on each of
//! `0xAD3CE0`), is thrown 150 back from its attacker where it may stand, and lies 5 s, doing
//! nothing else meanwhile (`hero::run`, `attack`, `making`); its marks clear while it stands in a
//! settlement at its whole health (`0x54808A`).
//!
//! Not followed: the interface's messages of the fall (`0x6770E0`, `0x6852B0`) and the object 874
//! at the hero (the FX event `0x62`).

use bevy::prelude::*;
use opensacred_rules::arts::KNOCKBACK_SPEED;
use opensacred_rules::fight::KNOCKED_BACK;

use crate::fight::{Downed, Health, KnockMarks};
use crate::frame::{Unpaused, WorldTick};
use crate::hero::{Body, Clips, GameSpeed};
use crate::terrain::Heights;

/// The ground's flag of a settlement (`+0x1E` bit 1, docs/02 §5.2), where a hero's marks clear.
const SETTLEMENT: u8 = 2;
/// How the fall blends in.
const BLEND: std::time::Duration = std::time::Duration::from_millis(200);

pub struct KnockDownPlugin;

impl Plugin for KnockDownPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (fall, lie, clear)
                .chain()
                .after(crate::fight::resolve)
                .after(crate::hero::run)
                .in_set(Unpaused),
        );
    }
}

/// The hero knocked down falls by its fall's clip at the rate 1.0, stopping, and each of its
/// near hostiles stops its strike (`0x54E6D0(1, 0)` on each of `0xAD3CE0`).
fn fall(
    mut commands: Commands,
    clips: Option<Res<Clips>>,
    speed: Res<GameSpeed>,
    nearest: Res<crate::crowd::Nearest>,
    mut heroes: Query<&mut Body, Added<Downed>>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for mut body in &mut heroes {
        body.stop();
        if let Some((node, _)) = clips.as_deref().and_then(|clips| clips.knocked.clone())
            && let Some(player) = clips.as_deref().and_then(|clips| clips.player)
            && let Ok((mut player, mut transitions)) = players.get_mut(player)
        {
            transitions
                .play(&mut player, node, BLEND)
                .set_speed(speed.0);
        }
        for &creature in &nearest.hostiles {
            commands
                .entity(creature)
                .try_remove::<crate::npcs::Strikes>();
        }
    }
}

/// The hero knocked down is thrown back from its attacker at the knocked creatures' speed until
/// it has gone 150, onto where it may stand (`+0x24D` and `+0x200` bit `0x20000`, as a kick's
/// knock, `making::Knocked`), and gets up after its 5 s.
fn lie(
    mut commands: Commands,
    time: Res<Time>,
    heights: Res<Heights>,
    mut heroes: Query<(Entity, &mut Transform, &Body, &mut Downed)>,
) {
    let seconds = time.delta_secs();
    for (hero, mut transform, body, mut downed) in &mut heroes {
        let here = transform.translation;
        let away = (here - downed.from).with_y(0.0).normalize_or_zero();
        if downed.thrown < KNOCKED_BACK && away != Vec3::ZERO {
            let step = (KNOCKBACK_SPEED * seconds).min(KNOCKED_BACK - downed.thrown);
            let to = here + away * step;
            let level = body.level();
            if heights.walkable_on(crate::paths::cell_of(to.x, to.z), level) {
                transform.translation.x = to.x;
                transform.translation.z = to.z;
                if let Some(y) = heights.standing(to.x, to.z, level) {
                    transform.translation.y = y;
                }
                downed.thrown += step;
            } else {
                downed.thrown = KNOCKED_BACK;
            }
        }
        downed.left -= seconds;
        if downed.left <= 0.0 {
            info!("knock_down: the hero gets up");
            commands.entity(hero).remove::<Downed>();
        }
    }
}

/// A hero's knock-down marks clear while it stands in a settlement at its whole health
/// (`0x54808A`, `0x4D4DA0`), at a tick of the world.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn clear(
    mut commands: Commands,
    tick: Res<WorldTick>,
    heights: Res<Heights>,
    heroes: Query<(Entity, &Transform, &Health), (With<KnockMarks>, With<Body>)>,
) {
    if tick.seconds.is_none() {
        return;
    }
    for (hero, transform, health) in &heroes {
        let here = transform.translation;
        let settled = heights
            .flags(crate::paths::cell_of(here.x, here.z))
            .is_some_and(|flags| flags & SETTLEMENT != 0);
        if settled && health.current >= health.most {
            commands.entity(hero).remove::<KnockMarks>();
        }
    }
}
