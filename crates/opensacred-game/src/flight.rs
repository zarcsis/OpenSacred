//! The flyers aloft (#2250, docs/re/ai.md, "The flyers"): an animal of a type that flies takes
//! off by its behaviour (`opensacred_rules::minds::Mind::behaves`), the hornets fly from their
//! making (#2569); a flyer's every walk is flown at its speed aloft ([`take_wing`]), climbing to
//! its height over the ground as it moves, at most half its distance to its target in melee, and
//! where it stands it comes down and lands, but the hornets, which keep flying on the ground
//! (`0x5941A0`); the terror and its death set it down at once (`0x44D520`).

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
        )
        .add_observer(take_wing);
    }
}

/// A walk of a flyer aloft is flown, whatever orders it (`+0x1F4` bit `0x200000`): at its speed
/// aloft, at the run's gait (`0x4F75F0`), onto the cells it flies onto and through the creatures
/// in them (`0x4EBCA0`, `0x4F7E10`), as a hornet's in melee (#2569).
fn take_wing(inserted: On<Insert<Walk>>, mut walks: Query<(&mut Walk, &Mind)>) {
    if let Ok((mut walk, mind)) = walks.get_mut(inserted.entity)
        && mind.flying
        && !walk.flies()
    {
        walk.take_wing(mind.fly);
    }
}

/// Raises each flyer over its ground: one that flies climbs as it moves, to its height
/// (`minds::flies_up_to`: in melee at most half the whole distance to the creature it fights or
/// the hero, `0x596600`), and comes down where it stands (`minds::climbs`), its model on its
/// place's ground plus its height; landed, its flying is over (`+0x1F4` bit `0x200000` cleared,
/// `0x598FDD`) and it stands in its idle clip (`npcs::stance`), but a hornet keeps flying on the
/// ground (`minds::lands`). One that flies no more, held by the terror or dead, is on the ground
/// at once.
/// A flyer is aloft from its flight's first frame; the walk that its behaviour gives it as it
/// takes off comes a frame after its flying.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn fly(
    mut commands: Commands,
    (time, speed, heights): (Res<Time>, Res<GameSpeed>, Res<Heights>),
    (heroes, targets): (
        Query<Entity, With<crate::hero::Body>>,
        Query<&GlobalTransform>,
    ),
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
    let hero = heroes.iter().next();
    for (entity, mut transform, mut mind, walk, aloft, way, dies) in &mut flyers {
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
        // In melee, its distance to the one it fights (`+0x104`), the creature or the hero.
        let aims = mind
            .as_deref()
            .filter(|mind| mind.fighting() && !mind.ranged)
            .and_then(|mind| {
                let at = targets.get(mind.foe().or(hero)?).ok()?.translation();
                Some((at - here).xz().length())
            });
        let creature = mind.as_deref().map_or(0, |mind| mind.creature);
        let high = minds::flies_up_to(creature, aims);
        // Its flying taken in melee, it comes down from where it is (#2577); a flyer again
        // comes down so no more.
        let descends = mind.as_deref().is_some_and(|mind| mind.descends);
        if let Some(mind) = mind.as_deref_mut().filter(|_| flying && descends) {
            mind.descends = false;
        }
        let next = if descends && !flying && !dies {
            height.and_then(|height| minds::descends(height, creature, seconds))
        } else {
            minds::climbs(
                height.unwrap_or(0.0),
                (flying, walk.is_some()),
                (seconds, high),
            )
        };
        match next {
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
                let keeps = flying && !minds::lands(creature);
                if let Some(mut mind) = mind
                    && !keeps
                {
                    mind.flying = false;
                    mind.descends = false;
                }
                if aloft.is_some() {
                    if keeps {
                        debug!("flight: {entity} is down on the ground, flying on");
                    } else {
                        debug!("flight: {entity} lands");
                    }
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
            .add_systems(Update, fly)
            .add_observer(take_wing);
        app
    }

    /// A hornet as a fill makes it, flying from its making (#2569).
    fn hornet(app: &mut App) -> Entity {
        let mut mind = Mind::new(
            Vec3::ZERO,
            crate::crowd::HOSTILE | crate::crowd::constructed(190, opensacred_rules::ways::FLIES),
            (false, false),
            None,
            [10.0, 20.0],
            (190, 2),
        );
        mind.flies = true;
        mind.fly = minds::FLIES_AT;
        app.world_mut().spawn((Transform::default(), mind)).id()
    }

    #[test]
    fn a_hornet_flies_its_walks_and_keeps_flying_on_the_ground() {
        let mut app = app();
        let hornet = hornet(&mut app);
        assert!(app.world().get::<Mind>(hornet).unwrap().flying);
        // Its walk, a plain one, is flown at its speed aloft.
        app.world_mut().entity_mut(hornet).insert(Walk::to(
            Vec3::new(5000.0, 0.0, 0.0),
            None,
            10.0,
        ));
        let walk = app.world().get::<Walk>(hornet).unwrap();
        assert!(walk.flies() && walk.runs());
        for _ in 0..4 {
            app.update();
        }
        let height = |app: &App| app.world().get::<Aloft>(hornet).map(|aloft| aloft.height);
        assert!(height(&app).is_some_and(|h| (h - 33.0).abs() < 1e-3));
        // Standing, it comes down to the ground and flies on there.
        app.world_mut().entity_mut(hornet).remove::<Walk>();
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(height(&app), None);
        assert!(app.world().get::<Mind>(hornet).unwrap().flying);
        // A walker that does not fly walks.
        let rabbit = crow(&mut app, false);
        app.world_mut()
            .entity_mut(rabbit)
            .insert(Walk::to(Vec3::X, None, 10.0));
        assert!(!app.world().get::<Walk>(rabbit).unwrap().flies());
    }

    #[test]
    fn a_hornet_in_melee_flies_at_most_half_its_distance_to_the_hero() {
        let mut app = app();
        let hornet = hornet(&mut app);
        app.world_mut().spawn((
            crate::hero::Body::standing(),
            GlobalTransform::from_translation(Vec3::new(0.0, 0.0, 97.0)),
        ));
        app.world_mut()
            .entity_mut(hornet)
            .insert(Aloft { height: 100.0 })
            .insert(Walk::to(Vec3::new(0.0, 0.0, 90.0), None, 10.0));
        // The first update's time is none.
        app.update();
        app.update();
        let height = |app: &App| app.world().get::<Aloft>(hornet).map(|aloft| aloft.height);
        // Not fighting it climbs on; fighting the hero 97 off, down to 48 at once.
        assert!(height(&app).is_some_and(|h| h > 100.0));
        assert!(app.world_mut().get_mut::<Mind>(hornet).unwrap().sent_at());
        app.update();
        assert_eq!(height(&app), Some(48.0));
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
    fn a_crow_whose_flying_melee_took_comes_down() {
        let mut app = app();
        let crow = crow(&mut app, false);
        app.world_mut().get_mut::<Mind>(crow).unwrap().descends = true;
        app.world_mut()
            .entity_mut(crow)
            .insert(Aloft { height: 50.0 })
            .insert(Transform::from_xyz(0.0, world(50.0), 0.0));
        let height = |app: &App| app.world().get::<Aloft>(crow).map(|aloft| aloft.height);
        // The first update's time is none; then 11 a tick.
        app.update();
        app.update();
        assert!(height(&app).is_some_and(|h| (h - 39.0).abs() < 1e-3));
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(height(&app), None);
        let mind = app.world().get::<Mind>(crow).unwrap();
        assert!(!mind.flying && !mind.descends);
        // One flying again comes down so no more.
        let again = crow_flying_again(&mut app);
        assert!(!app.world().get::<Mind>(again).unwrap().descends);
    }

    fn crow_flying_again(app: &mut App) -> Entity {
        let again = crow(app, true);
        app.world_mut().get_mut::<Mind>(again).unwrap().descends = true;
        app.world_mut()
            .entity_mut(again)
            .insert(Aloft { height: 50.0 });
        app.update();
        again
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
