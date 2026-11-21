//! A player's hero knocked down by a heavy blow (#2311; docs/re/fight.md, "A knock-down"): the
//! strike's resolution marks it ([`crate::fight::Downed`], `0x56E366`–`0x56E8CC`); here it falls
//! by its slot 228 at the rate 1.0, its near hostiles' strikes stopping (`0x54E6D0` on each of
//! `0xAD3CE0`), is thrown 150 back from its attacker where it may stand, and lies 5 s, doing
//! nothing else meanwhile (`hero::run`, `attack`, `making`); its marks clear while it stands in a
//! settlement at its whole health (`0x54808A`).
//!
//! A player's hero knocked back by a creature's whirlwind (#2901, [`Pushed`]): it leaves what it
//! does, stands in its idle clip and slides away from the whirlwind's caster until it has gone the knock's
//! distance, which each new strike sets anew (the event of the kind 3, `0x531AF7`, which the
//! original's hero takes as a creature does; seen 2026-11-19).
//!
//! The fall's voice line of one that falls unconscious (`0x6770E0`, `0x6852B0`) is not spoken,
//! no hero's voice holding one (a companion's is #3087). Its butterflies are `knockouts`' (the
//! object 874, #3023). Not followed yet: its turn to its attacker (#3079).

use bevy::prelude::*;
use opensacred_rules::arts::KNOCKBACK_SPEED;
use opensacred_rules::fight::KNOCKED_BACK;

use crate::fight::{Downed, Health, KnockMarks, Pushed};
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
            (fall, lie, pushed, slide, clear)
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

/// The hero knocked back by a whirlwind (#2901) leaves what it does as it is first knocked, its
/// walk, its attack and its art, and stands in its idle clip (`0x54E6D0(0, 1)`).
fn pushed(
    mut commands: Commands,
    clips: Option<Res<Clips>>,
    speed: Res<GameSpeed>,
    mut heroes: Query<(Entity, &mut Body), Added<Pushed>>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for (hero, mut body) in &mut heroes {
        body.stop();
        commands.entity(hero).try_remove::<(
            crate::attack::Attack,
            crate::attack::Striking,
            crate::making::Making,
            crate::making::Playing,
        )>();
        if let Some(clips) = clips.as_deref()
            && let Some(player) = clips.player
            && let Ok((mut player, mut transitions)) = players.get_mut(player)
        {
            transitions
                .play(&mut player, clips.idle(), BLEND)
                .set_speed(speed.0);
        }
    }
}

/// Slides the hero knocked back away from its whirlwind's caster (#3305), at the knocked
/// creatures' speed, until it has gone what is left (`0x594D12`, as `making::Knocked`), onto
/// where it may stand, a cell it may not enter ending it; then it is free.
fn slide(
    mut commands: Commands,
    time: Res<Time>,
    heights: Res<Heights>,
    mut heroes: Query<(Entity, &mut Transform, &Body, &mut Pushed)>,
) {
    let seconds = time.delta_secs();
    for (hero, mut transform, body, mut pushed) in &mut heroes {
        let here = transform.translation;
        let away = (here - pushed.from).with_y(0.0).normalize_or_zero();
        let step = (KNOCKBACK_SPEED * seconds).min(pushed.left);
        let to = here + away * step;
        let level = body.level();
        if away == Vec3::ZERO || !heights.walkable_on(crate::paths::cell_of(to.x, to.z), level) {
            pushed.left = 0.0;
        } else {
            transform.translation.x = to.x;
            transform.translation.z = to.z;
            if let Some(y) = heights.standing(to.x, to.z, level) {
                transform.translation.y = y;
            }
            pushed.left -= step;
        }
        if pushed.left <= 0.0 {
            commands.entity(hero).remove::<Pushed>();
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::time::TimeUpdateStrategy;
    use opensacred_data::world::{Cell, SIDE, Sector};

    use super::*;

    /// Sector (0, 0) walkable whole.
    fn open() -> Heights {
        let cell = Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground: 0x40,
        };
        let mut heights = Heights::default();
        heights.add(&Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells: vec![cell; SIDE * SIDE],
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: Vec::new(),
        });
        heights
    }

    #[test]
    fn a_hero_knocked_back_slides_away_from_the_whirlwind_its_distance_and_is_free() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                50,
            )))
            .insert_resource(open())
            .insert_resource(GameSpeed(1.0))
            .add_systems(Update, (pushed, slide).chain());
        let start = crate::iso::ground(20.5, 20.5);
        let hero = app
            .world_mut()
            .spawn((Transform::from_translation(start), Body::standing()))
            .id();
        app.update();
        app.world_mut().entity_mut(hero).insert(Pushed {
            from: start - Vec3::X * 10.0,
            left: 80.0,
        });
        for _ in 0..10 {
            app.update();
        }
        assert!(app.world().get::<Pushed>(hero).is_none());
        let moved = app.world().get::<Transform>(hero).unwrap().translation - start;
        assert!(
            (moved.x - 80.0).abs() < 0.01 && moved.z.abs() < 0.01,
            "{moved}"
        );
    }
}
