//! The hero held (#1852; docs/re/ai.md, "The creatures' spells"): while the hero bears the effect
//! 15 (`opensacred_rules::spells::HELD`), which a creature's flesh to stone gives it, its time's
//! factor 0 (`0x583485`, `0x537160`) freezes it where it stands: its clip stands still, and its
//! orders are dropped as they come, its walk, its attack, its art and its pick-up, so that it
//! takes up none of them once free, as the original's hero did not (seen 2026-11-02).

use bevy::prelude::*;

use crate::frame::Step;
use crate::hero::{Body, Clips};
use crate::npcs::Dies;

/// The hero while it is held.
#[derive(Component, Debug)]
pub struct HeroHeld;

/// Whether `effects` hold the hero.
fn holds(effects: Option<&crate::effects::Effects>) -> bool {
    effects.is_some_and(|effects| effects.holds(opensacred_rules::spells::HELD))
}

/// Holds the hero while it bears the effect 15, and lets it go after: each frame its orders
/// dropped and its clip paused, and its clip resumed once the effect ends.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn hold(
    mut commands: Commands,
    clips: Option<Res<Clips>>,
    mut heroes: Query<
        (
            Entity,
            &mut Body,
            Option<&crate::effects::Effects>,
            Has<HeroHeld>,
        ),
        Without<Dies>,
    >,
    mut players: Query<&mut AnimationPlayer>,
) {
    let mut player = clips
        .as_ref()
        .and_then(|clips| clips.player)
        .and_then(|player| players.get_mut(player).ok());
    for (hero, mut body, effects, was) in &mut heroes {
        let held = holds(effects);
        if held {
            if !was {
                info!("hero_held: the hero is held");
                commands.entity(hero).insert(HeroHeld);
            }
            body.stop();
            commands.entity(hero).remove::<(
                crate::attack::Attack,
                crate::attack::Striking,
                crate::making::Making,
                crate::making::Playing,
                crate::pickup::Picking,
            )>();
            if let Some(player) = player.as_mut() {
                player.pause_all();
            }
        } else if was {
            info!("hero_held: the hero is free again");
            commands.entity(hero).remove::<HeroHeld>();
            if let Some(player) = player.as_mut() {
                player.resume_all();
            }
        }
    }
}

/// The hero held.
pub struct HeroHeldPlugin;

impl Plugin for HeroHeldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            hold.before(crate::hero::take_orders)
                .after(crate::hero::steer)
                .in_set(Step::HeroMove),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stone_holds_the_hero_dropping_its_orders_until_it_ends() {
        let mut app = App::new();
        app.add_systems(Update, hold);
        let stone = opensacred_rules::fight::Effect {
            id: opensacred_rules::spells::HELD,
            whole: 0,
            value: 0.0,
            art: u32::from(opensacred_rules::spells::STONE),
            level: 0,
        };
        let mut effects = crate::effects::Effects::default();
        effects.add(stone, 17.0);
        let mut body = Body::standing();
        body.send(Vec3::X);
        let hero = app.world_mut().spawn((body, effects)).id();
        app.update();
        let world = app.world();
        assert!(world.get::<HeroHeld>(hero).is_some());
        assert!(!world.get::<Body>(hero).is_some_and(Body::ordered));
        // An order given while held is dropped as it comes.
        let send = |app: &mut App| {
            if let Some(mut body) = app.world_mut().get_mut::<Body>(hero) {
                body.send(Vec3::Z);
            }
        };
        send(&mut app);
        app.update();
        assert!(!app.world().get::<Body>(hero).is_some_and(Body::ordered));
        // The effect over, it is free, and an order stands.
        if let Some(mut effects) = app.world_mut().get_mut::<crate::effects::Effects>(hero) {
            effects.remove(opensacred_rules::spells::HELD);
        }
        app.update();
        send(&mut app);
        app.update();
        let world = app.world();
        assert!(world.get::<HeroHeld>(hero).is_none());
        assert!(world.get::<Body>(hero).is_some_and(Body::ordered));
    }
}
