//! The flying animals aloft (#2250, docs/re/ai.md, "The flyers"): an animal of a type that flies
//! takes off by its behaviour (`opensacred_rules::minds::Mind::behaves`) and flies at its speed
//! aloft (`npcs::Walk::fly`), climbing to its height over the ground as it moves, and where it
//! stands it comes down and lands (`0x5941A0`); the terror and its death set it down at once
//! (`0x44D520`).

use bevy::prelude::*;
use opensacred_rules::minds;

use crate::ai::Mind;
use crate::crowd::InTheWay;
use crate::frame::Step;
use crate::hero::GameSpeed;
use crate::iso;
use crate::npcs::{Dies, Walk};
use crate::terrain::Heights;

/// A flyer off the ground: its height over it, in the units of the models' height, as the
/// original's heights are.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct Aloft {
    pub height: f32,
}

/// World units of a height in the units of the models' height, raised on the screen as the
/// original raises it, as the ground's heights are (`terrain`).
fn world(height: f32) -> f32 {
    iso::height(height * iso::PIXELS_PER_MODEL_HEIGHT)
}

pub struct FlightPlugin;

impl Plugin for FlightPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            fly.after(crate::npcs::walk)
                .before(crate::npcs::stance)
                .in_set(Step::Npcs),
        );
    }
}

/// Raises each flyer over its ground: one that flies climbs as it moves and comes down where it
/// stands (`minds::climbs`), its model on its place's ground plus its height; landed, its flying
/// is over (`+0x1F4` bit `0x200000` cleared, `0x598FDD`) and it stands in its idle clip
/// (`npcs::stance`). One that flies no more, held by the terror or dead, is on the ground at once.
/// A flyer is aloft from its flight's first frame; the walk that its behaviour gives it as it
/// takes off comes a frame after its flying.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn fly(
    mut commands: Commands,
    (time, speed, heights): (Res<Time>, Res<GameSpeed>, Res<Heights>),
    mut flyers: Query<
        (
            Entity,
            &mut Transform,
            Option<&mut Mind>,
            Option<&Walk>,
            Option<&mut Aloft>,
            Option<&InTheWay>,
            Has<Dies>,
        ),
        Or<(With<Aloft>, With<Walk>)>,
    >,
) {
    let seconds = time.delta_secs() * speed.0;
    for (entity, mut transform, mind, walk, aloft, way, dies) in &mut flyers {
        let height = aloft.as_deref().map(|aloft| aloft.height);
        if height.is_none() && !walk.is_some_and(Walk::flies) {
            continue;
        }
        let here = transform.translation;
        let level = way.and_then(|way| way.level);
        let ground = heights
            .standing(here.x, here.z, level)
            .unwrap_or(here.y - world(height.unwrap_or(0.0)));
        let flying = !dies && mind.as_deref().is_some_and(|mind| mind.flying);
        match minds::climbs(height.unwrap_or(0.0), (flying, walk.is_some()), seconds) {
            Some(height) => {
                match aloft {
                    Some(mut aloft) => aloft.height = height,
                    None => {
                        commands.entity(entity).insert(Aloft { height });
                    }
                }
                transform.translation.y = ground + world(height);
            }
            None => {
                if let Some(mut mind) = mind {
                    mind.flying = false;
                }
                if aloft.is_some() {
                    debug!("flight: {entity} lands");
                    commands.entity(entity).remove::<Aloft>();
                }
                transform.translation.y = ground;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::time::TimeUpdateStrategy;

    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .insert_resource(GameSpeed(1.0))
            .insert_resource(Heights::default())
            .add_systems(Update, fly);
        app
    }

    fn crow(app: &mut App, flying: bool) -> Entity {
        let mut mind = Mind::new(
            Vec3::ZERO,
            minds::of_type(513),
            (true, false),
            None,
            [10.0, 200.0],
            (513, 6),
        );
        mind.flies = true;
        mind.flying = flying;
        app.world_mut().spawn((Transform::default(), mind)).id()
    }

    #[test]
    fn a_crow_climbs_as_it_flies_and_lands_where_it_stands() {
        let mut app = app();
        let crow = crow(&mut app, true);
        app.world_mut().entity_mut(crow).insert(Walk::fly(
            Vec3::new(5000.0, 0.0, 0.0),
            None,
            200.0,
        ));
        // The first update's time is none; then 11 a tick, to 120.
        for _ in 0..20 {
            app.update();
        }
        let height = |app: &App| app.world().get::<Aloft>(crow).map(|aloft| aloft.height);
        assert_eq!(height(&app), Some(minds::FLIES_HIGH));
        let y = app.world().get::<Transform>(crow).unwrap().translation.y;
        assert!((y - world(minds::FLIES_HIGH)).abs() < 1e-3, "{y}");
        // Standing, it comes down, 11 a tick, and lands.
        app.world_mut().entity_mut(crow).remove::<Walk>();
        app.update();
        assert!(height(&app).is_some_and(|h| (h - 109.0).abs() < 1e-3));
        for _ in 0..12 {
            app.update();
        }
        assert_eq!(height(&app), None);
        assert!(!app.world().get::<Mind>(crow).unwrap().flying);
        assert!(
            app.world()
                .get::<Transform>(crow)
                .unwrap()
                .translation
                .y
                .abs()
                < 1e-3
        );
    }

    #[test]
    fn a_crow_that_flies_no_more_is_on_the_ground_at_once() {
        let mut app = app();
        let crow = crow(&mut app, false);
        app.world_mut()
            .entity_mut(crow)
            .insert(Aloft { height: 80.0 })
            .insert(Transform::from_xyz(0.0, world(80.0), 0.0));
        app.update();
        assert!(app.world().get::<Aloft>(crow).is_none());
        assert!(
            app.world()
                .get::<Transform>(crow)
                .unwrap()
                .translation
                .y
                .abs()
                < 1e-3
        );
        // A walker on the ground is left alone.
        let rabbit = app
            .world_mut()
            .spawn((
                Transform::from_xyz(0.0, 5.0, 0.0),
                Walk::to(Vec3::X, None, 10.0),
            ))
            .id();
        app.update();
        assert!(app.world().get::<Aloft>(rabbit).is_none());
        assert_eq!(
            app.world().get::<Transform>(rabbit).unwrap().translation.y,
            5.0
        );
    }
}
